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
use language_graph_engine::seed::lowercase_latin::COLLECTION_ENTITY_ID as LOW_COL_ID;
use language_graph_engine::seed::phase2::{
    seed_phase2, DIGITS_COLLECTION_ENTITY_ID, PROFILE_ENTITY_ID, PUNCTUATION_COLLECTION_ENTITY_ID,
    UPPERCASE_COLLECTION_ENTITY_ID, WHITESPACE_COLLECTION_ENTITY_ID,
};
use language_graph_engine::server::Server;
use proptest::prelude::*;
use rusqlite::Connection;
use tower::ServiceExt;
use unicode_normalization::UnicodeNormalization;

const GOLDEN_A_REV_CID: &str = "bafyreigzc6usxy4ufmz43vpotqo54cqrdqwtzgebsxwhyybmkgphbfwq5a";
const GOLDEN_Z_REV_CID: &str = "bafyreiamiyi7szcus6c67balumi6ejdvby5jiad5yr375sveyufkkb63dm";
const GOLDEN_LOW_SNAP_CID: &str = "bafyreib4ivpoazb5skkr7yvfelvoowz6sxxncdsjewvxawyedm5tikeshm";

const GOLDEN_UPPER_A_CID: &str = "bafyreievw56s5ltwde2xmmxzt3etkfz73qsutx43x7xuxe3iuvnbpobm2e";
const GOLDEN_UPPER_Z_CID: &str = "bafyreiezwovnoifhtfjxgtotbszxaku5bkc4br7vdjopukdmxhtbbbju2y";
const GOLDEN_DIGIT_0_CID: &str = "bafyreihed7ebhkvf5b27tyuhzwsdp3bcrjdxiwow2idla4rvw5qskorbxe";
const GOLDEN_DIGIT_9_CID: &str = "bafyreihjhfma5buevawmzrchj3bauifuvc5ey5b2x4fgfsjmiqqease33u";
const GOLDEN_SPACE_CID: &str = "bafyreife2nx5traghcw3frzjh4wo2rb2ww5cybvptdz6c3ayn2ukioagnq";
const GOLDEN_PUNCT_DOT_CID: &str = "bafyreiecsu3larivauz7adpncch22ml26srmct2jvtfv3jc4w73bi3omvu";
const GOLDEN_PUNCT_APOS_CID: &str = "bafyreia4iwm7sbr6rirxz5rhtn23h6awmrivwobujbgyyx66rqvdksqfam";
const GOLDEN_PUNCT_QUOTE_CID: &str = "bafyreiemmlqku44pnvm2tbxlfuaocoypp4eqxukzgspw4zhyuglixyh6du";
const GOLDEN_PUNCT_EXCL_CID: &str = "bafyreieaz24s7nzw54rchufcv4btgmotic3j6tr7is4gbmj7ul74b57wdm";

const GOLDEN_UPPER_SNAP_CID: &str = "bafyreie5eeznusjiimg7l666feoxwvvk62pbhs6hb7cryh2ana53gm2uqm";
const GOLDEN_DIGIT_SNAP_CID: &str = "bafyreig3oqpkm3gpsmcs7f25u4vmyvkzczwhjqfvs4iumwpi5uelb353s4";
const GOLDEN_SPACE_SNAP_CID: &str = "bafyreidh3pi73vtvgkpt7gbbpbbx2lbi42yracmlnagtfljkfmi4mv36ky";
const GOLDEN_PUNCT_SNAP_CID: &str = "bafyreidh6nez45kqwkc5ue6c5fvhcblmtehlbae5ubiafmt77rpazrzyuq";
const GOLDEN_PROFILE_SNAP_CID: &str = "bafyreic5acpnm6zr4cp6jl3xm425kwft77qegml2mhxftrwclkelnqplry";

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
fn test_regression_phase1_cids_unchanged() {
    let mut conn = get_temp_db();
    let profile_cid = seed_phase2(&mut conn).unwrap();
    assert_eq!(profile_cid, GOLDEN_PROFILE_SNAP_CID);

    let repo = Repository::new(&conn);

    // Verify Phase 1 snapshots and revisions exist and match golden CIDs
    let low_snap_cid = repo.get_active_snapshot_cid(LOW_COL_ID).unwrap().unwrap();
    assert_eq!(low_snap_cid, GOLDEN_LOW_SNAP_CID);

    let members = repo.get_snapshot_members(&low_snap_cid).unwrap();
    assert_eq!(members.len(), 26);
    assert_eq!(members[0].revision_cid, GOLDEN_A_REV_CID);
    assert_eq!(members[25].revision_cid, GOLDEN_Z_REV_CID);
}

#[test]
fn test_deterministic_new_symbol_cids() {
    let test_cases = vec![
        ('A', "Latn", "uppercase", GOLDEN_UPPER_A_CID),
        ('Z', "Latn", "uppercase", GOLDEN_UPPER_Z_CID),
        ('0', "Common", "none", GOLDEN_DIGIT_0_CID),
        ('9', "Common", "none", GOLDEN_DIGIT_9_CID),
        (' ', "Common", "none", GOLDEN_SPACE_CID),
        ('.', "Common", "none", GOLDEN_PUNCT_DOT_CID),
        ('\'', "Common", "none", GOLDEN_PUNCT_APOS_CID),
        ('\"', "Common", "none", GOLDEN_PUNCT_QUOTE_CID),
        ('!', "Common", "none", GOLDEN_PUNCT_EXCL_CID),
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
    seed_phase2(&mut conn).unwrap();
    let repo = Repository::new(&conn);

    let upper_snap = repo
        .get_active_snapshot_cid(UPPERCASE_COLLECTION_ENTITY_ID)
        .unwrap()
        .unwrap();
    assert_eq!(upper_snap, GOLDEN_UPPER_SNAP_CID);
    let upper_members = repo.get_snapshot_members(&upper_snap).unwrap();
    assert_eq!(upper_members.len(), 26);
    assert_eq!(
        upper_members[0].entity_id,
        "urn:language-graph:grapheme:nfc:0041"
    ); // 'A'
    assert_eq!(
        upper_members[25].entity_id,
        "urn:language-graph:grapheme:nfc:005a"
    ); // 'Z'

    let digit_snap = repo
        .get_active_snapshot_cid(DIGITS_COLLECTION_ENTITY_ID)
        .unwrap()
        .unwrap();
    assert_eq!(digit_snap, GOLDEN_DIGIT_SNAP_CID);
    let digit_members = repo.get_snapshot_members(&digit_snap).unwrap();
    assert_eq!(digit_members.len(), 10);
    assert_eq!(
        digit_members[0].entity_id,
        "urn:language-graph:grapheme:nfc:0030"
    ); // '0'
    assert_eq!(
        digit_members[9].entity_id,
        "urn:language-graph:grapheme:nfc:0039"
    ); // '9'

    let space_snap = repo
        .get_active_snapshot_cid(WHITESPACE_COLLECTION_ENTITY_ID)
        .unwrap()
        .unwrap();
    assert_eq!(space_snap, GOLDEN_SPACE_SNAP_CID);
    let space_members = repo.get_snapshot_members(&space_snap).unwrap();
    assert_eq!(space_members.len(), 1);
    assert_eq!(
        space_members[0].entity_id,
        "urn:language-graph:grapheme:nfc:0020"
    ); // ' '

    let punct_snap = repo
        .get_active_snapshot_cid(PUNCTUATION_COLLECTION_ENTITY_ID)
        .unwrap()
        .unwrap();
    assert_eq!(punct_snap, GOLDEN_PUNCT_SNAP_CID);
    let punct_members = repo.get_snapshot_members(&punct_snap).unwrap();
    assert_eq!(punct_members.len(), 11);
    assert_eq!(
        punct_members[0].entity_id,
        "urn:language-graph:grapheme:nfc:002e"
    ); // '.'
    assert_eq!(
        punct_members[3].entity_id,
        "urn:language-graph:grapheme:nfc:0021"
    ); // '!'
}

#[test]
fn test_text_profile_snapshot_cid() {
    let mut conn = get_temp_db();
    let profile_cid = seed_phase2(&mut conn).unwrap();
    assert_eq!(profile_cid, GOLDEN_PROFILE_SNAP_CID);

    let repo = Repository::new(&conn);
    let active_profile_cid = repo
        .get_active_profile_snapshot_cid(PROFILE_ENTITY_ID)
        .unwrap()
        .unwrap();
    assert_eq!(active_profile_cid, GOLDEN_PROFILE_SNAP_CID);

    let profile = repo.get_profile_snapshot(&active_profile_cid).unwrap();
    assert_eq!(profile.schema, "language-graph/text-profile-snapshot/v1");
    assert_eq!(profile.profile_entity_id, PROFILE_ENTITY_ID);
    assert_eq!(profile.kind, "written-text-profile");
    assert_eq!(profile.label, "Basic English Written Text Profile");
    assert_eq!(profile.collections.len(), 5);

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
}

#[test]
fn test_seeding_idempotency() {
    let mut conn = get_temp_db();
    let cid1 = seed_phase2(&mut conn).unwrap();
    let cid2 = seed_phase2(&mut conn).unwrap();
    assert_eq!(cid1, cid2);

    let block_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM immutable_blocks", [], |row| {
            row.get(0)
        })
        .unwrap();
    // 26 low + 26 upper + 10 digits + 1 space + 11 punct = 74 graphemes
    // 5 collection snapshots + 1 profile snapshot = 6 snapshots
    // 74 + 6 = 80 total blocks
    assert_eq!(block_count, 80);
}

#[test]
fn test_transaction_safety_on_phase2_failure() {
    let mut conn = get_temp_db();

    // Inject a conflicting entity for exclamation point '!' before seeding
    conn.execute(
        "INSERT INTO entities (entity_id, kind, canonical_key, label, created_at)
         VALUES ('urn:language-graph:grapheme:nfc:0021', 'grapheme', '!', 'Conflict Exclamation', datetime('now'))",
        [],
    ).unwrap();

    let res = seed_phase2(&mut conn);
    assert!(res.is_err());

    // Transaction should roll back and not create the profile table entries
    let repo = Repository::new(&conn);
    let active_profile = repo
        .get_active_profile_snapshot_cid(PROFILE_ENTITY_ID)
        .unwrap();
    assert!(active_profile.is_none());
}

#[test]
fn test_resolver_valid_cases() {
    let mut conn = get_temp_db();
    seed_phase2(&mut conn).unwrap();

    let resolver = TextResolver::load(&conn).unwrap();
    assert_eq!(resolver.cache.len(), 74);

    let test_inputs = vec![
        "Hello, Vatsal!",
        "Room 101.",
        "I can't wait.",
        "Is this working?",
        "(Phase 2) - 2026!",
        "abcdefghijklmnopqrstuvwxyz",
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        "0123456789",
    ];

    for input in test_inputs {
        let res = resolver.resolve(input).unwrap();
        assert_eq!(res.input, input);
        assert_eq!(res.output, input);
        assert_eq!(res.collection_snapshot_cid, GOLDEN_PROFILE_SNAP_CID);

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
fn test_resolver_banana_reuse_and_trace_status() {
    let mut conn = get_temp_db();
    seed_phase2(&mut conn).unwrap();

    let resolver = TextResolver::load(&conn).unwrap();
    let res = resolver.resolve("banana").unwrap();

    assert_eq!(res.trace[0].input_grapheme, "b");
    assert_eq!(res.trace[0].status, "Resolved");

    assert_eq!(res.trace[1].input_grapheme, "a");
    assert_eq!(res.trace[1].status, "Resolved");

    assert_eq!(res.trace[2].input_grapheme, "n");
    assert_eq!(res.trace[2].status, "Resolved");

    assert_eq!(res.trace[3].input_grapheme, "a");
    assert_eq!(res.trace[3].status, "Reused");
}

#[test]
fn test_resolver_unsupported_validation_errors() {
    let mut conn = get_temp_db();
    seed_phase2(&mut conn).unwrap();

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
        (
            "café",
            vec!["é U+00e9 LATIN SMALL LETTER E WITH ACUTE at position 4"],
        ),
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
    seed_phase2(&mut conn).unwrap();

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

    // Resolve valid text and invalid text
    let _ = resolver.resolve("Hello, Vatsal!");
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
    fn test_property_resolver_identity(s in "[a-zA-Z0-9.,?!'\"\\-:;() ]{1,100}") {
        let mut conn = get_temp_db();
        seed_phase2(&mut conn).unwrap();
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
        seed_phase: "phase2".to_string(),
    };
    let app_state = AppState::new(config).expect("AppState init failed");
    let app = Server::build_router(app_state);
    (app, temp_db)
}

#[tokio::test]
async fn test_http_api_phase2_endpoints() {
    let (app, _temp_db) = get_test_app().await;

    // 1. GET /api/status returns 74 count and active profile snapshot CID
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
    assert_eq!(status_json["symbolCount"], 74);
    assert_eq!(status_json["activeSnapshotCid"], GOLDEN_PROFILE_SNAP_CID);

    // 2. GET /api/collections returns 5 collections
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
    assert_eq!(arr.len(), 5);

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
    assert_eq!(profile["profileEntityId"], PROFILE_ENTITY_ID);

    // 4. POST /api/resolve accepts valid Phase 2 input
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/resolve")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "text": "Hello, Vatsal!" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let resolve_res: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(resolve_res["output"], "Hello, Vatsal!");
}
