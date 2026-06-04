use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use rusqlite::Connection;
use serde_json::Value;
use tower::ServiceExt;

use language_graph_engine::app::AppState;
use language_graph_engine::config::Config;
use language_graph_engine::seed::lowercase_latin::COLLECTION_ENTITY_ID;
use language_graph_engine::server::Server;

async fn get_test_app() -> (axum::Router, tempfile::NamedTempFile) {
    let temp_db = tempfile::NamedTempFile::new().expect("Failed to create temp DB file");
    let config = Config {
        db_path: temp_db.path().to_path_buf(),
        listen_port: 0, // In-process test won't bind to port
    };
    let app_state = AppState::new(config).expect("AppState init failed");
    let app = Server::build_router(app_state);
    (app, temp_db)
}

#[tokio::test]
async fn test_http_api_endpoints() {
    let (app, _temp_db) = get_test_app().await;

    // 1. GET / returns success and serves public/index.html containing "Language Graph Engine"
    let response = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(
        body_str.contains("Language Graph Engine"),
        "Body was: {}",
        body_str
    );

    // 2. GET /api/status returns active snapshot CID and 26-member count
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
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let status_json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(status_json["symbolCount"], 26);
    assert_eq!(
        status_json["identifierFormat"],
        "CIDv1 / DAG-CBOR / SHA2-256"
    );
    let active_cid = status_json["activeSnapshotCid"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(!active_cid.is_empty());

    // 3. GET /api/symbols returns exactly 26 healthy active symbols
    // 4. Symbols returned in canonical order 'a' through 'z'
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/symbols")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let symbols_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    let symbols_arr = symbols_json.as_array().unwrap();
    assert_eq!(symbols_arr.len(), 26);

    for (i, sym) in symbols_arr.iter().enumerate().take(26) {
        assert_eq!(sym["position"], (i + 1) as i64);
        let ch = (b'a' + i as u8) as char;
        assert_eq!(sym["surfaceForm"], ch.to_string());
        assert_eq!(sym["status"], "Healthy");
    }

    // Check symbols details for 'a'
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/symbols/urn:language-graph:grapheme:nfc:0061")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let details_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(
        details_json["entityId"],
        "urn:language-graph:grapheme:nfc:0061"
    );
    assert_eq!(details_json["surfaceForm"], "a");
    assert_eq!(details_json["codec"], "dag-cbor");
    assert_eq!(details_json["multihashFormat"], "sha2-256");

    // 5. Active snapshot details return exactly 26 ordered members
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/snapshots/active")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let snap_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(snap_json["collectionEntityId"], COLLECTION_ENTITY_ID);
    assert_eq!(snap_json["members"].as_array().unwrap().len(), 26);

    // 6. POST /api/resolve for 'vatsal' returns output and trace rows
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/resolve")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "text": "vatsal" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let resolve_ok_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(resolve_ok_json["output"], "vatsal");
    assert_eq!(resolve_ok_json["collectionSnapshotCid"], active_cid);
    let trace = resolve_ok_json["trace"].as_array().unwrap();
    assert_eq!(trace.len(), 6);
    assert_eq!(trace[1]["inputGrapheme"], "a");
    assert_eq!(trace[1]["status"], "Resolved");
    assert_eq!(trace[4]["inputGrapheme"], "a");
    assert_eq!(trace[4]["status"], "Reused");

    // 7. POST /api/resolve for 'banana' returns trace rows and reused statuses
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/resolve")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "text": "banana" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let resolve_banana_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(resolve_banana_json["output"], "banana");
    let trace_banana = resolve_banana_json["trace"].as_array().unwrap();
    assert_eq!(trace_banana[0]["inputGrapheme"], "b");
    assert_eq!(trace_banana[0]["status"], "Resolved");
    assert_eq!(trace_banana[3]["inputGrapheme"], "a");
    assert_eq!(trace_banana[3]["status"], "Reused");

    // 8. POST /api/resolve rejects invalid input with structured client-facing validation errors
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/resolve")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "text": "banana1" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let resolve_err_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(resolve_err_json["error"]
        .as_str()
        .unwrap()
        .contains("Unsupported character or grapheme: '1'"));
}

#[tokio::test]
async fn test_http_api_database_integrity_failure_and_non_mutation() {
    let (app, temp_db) = get_test_app().await;

    // First fetch a valid symbol details
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/symbols/urn:language-graph:grapheme:nfc:0061")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Record row counts of database
    let conn = Connection::open(temp_db.path()).unwrap();
    let get_row_counts = |c: &Connection| -> (i64, i64, i64, i64, i64, i64, i64) {
        let blocks: i64 = c
            .query_row("SELECT COUNT(*) FROM immutable_blocks", [], |r| r.get(0))
            .unwrap();
        let entities: i64 = c
            .query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))
            .unwrap();
        let heads: i64 = c
            .query_row("SELECT COUNT(*) FROM entity_heads", [], |r| r.get(0))
            .unwrap();
        let collections: i64 = c
            .query_row("SELECT COUNT(*) FROM collections", [], |r| r.get(0))
            .unwrap();
        let snapshots: i64 = c
            .query_row("SELECT COUNT(*) FROM collection_snapshots", [], |r| {
                r.get(0)
            })
            .unwrap();
        let members: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM collection_snapshot_members",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let active_snaps: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM active_collection_snapshots",
                [],
                |r| r.get(0),
            )
            .unwrap();
        (
            blocks,
            entities,
            heads,
            collections,
            snapshots,
            members,
            active_snaps,
        )
    };
    let counts_before = get_row_counts(&conn);

    // Query status, resolve a word, query active snapshot - none of these should mutate tables
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/resolve")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "text": "banana" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/snapshots/active")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let counts_after = get_row_counts(&conn);
    assert_eq!(
        counts_before, counts_after,
        "API read requests must not mutate tables"
    );

    // 9. Deliberately corrupt the database store and verify that fetching triggers a controlled conflict/integrity response
    conn.execute(
        "UPDATE immutable_blocks SET bytes = x'ffffff' WHERE cid = 'bafyreigzc6usxy4ufmz43vpotqo54cqrdqwtzgebsxwhyybmkgphbfwq5a'",
        [],
    ).unwrap();

    // Now request details of symbol 'a'.
    // The server should return CONFLICT (409) rather than panicking or leaking a raw stack trace.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/symbols/urn:language-graph:grapheme:nfc:0061")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let err_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(err_json["error"]
        .as_str()
        .unwrap()
        .contains("Integrity check failed"));
}
