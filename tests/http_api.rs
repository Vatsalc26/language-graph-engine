use tokio::net::TcpListener;
use language_graph_engine::config::Config;
use language_graph_engine::app::AppState;
use language_graph_engine::server::Server;
use serde_json::Value;

// Helper to start the test server on an ephemeral port
async fn start_test_server() -> (String, tempfile::NamedTempFile) {
    // Create a temporary database file
    let temp_db = tempfile::NamedTempFile::new().expect("Failed to create temporary DB file");
    
    // Create config on a random free port and the temp db
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("Failed to bind ephemeral port");
    let addr = listener.local_addr().expect("Failed to get local address");
    let port = addr.port();
    drop(listener);

    let config = Config {
        db_path: temp_db.path().to_path_buf(),
        listen_port: port,
    };

    let app_state = AppState::new(config).expect("Failed to initialize AppState");
    
    // Spawn the real server in the background
    let app_state_clone = app_state.clone();
    tokio::spawn(async move {
        let _ = Server::run(app_state_clone).await;
    });

    // Give the server a small moment to start
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    (format!("http://127.0.0.1:{}", port), temp_db)
}

#[tokio::test]
async fn test_http_endpoints() {
    // Ensure public folder and a dummy index.html exists so fallback serving can be verified
    std::fs::create_dir_all("public").expect("Failed to create public dir");
    std::fs::write("public/index.html", "<html><body>Language Graph Engine</body></html>")
        .expect("Failed to write index.html");

    let (base_url, _temp_db) = start_test_server().await;
    let client = reqwest::Client::new();

    // 1. GET / serves main browser interface successfully
    let res_index = client.get(&base_url).send().await.expect("Failed to GET /");
    assert_eq!(res_index.status(), reqwest::StatusCode::OK);
    let body_index = res_index.text().await.expect("Failed to read index body");
    assert!(body_index.contains("Language Graph Engine"));

    // 2. GET /api/status returns active snapshot CID and 26-member count
    let res_status = client.get(format!("{}/api/status", base_url)).send().await.expect("Failed to GET /api/status");
    assert_eq!(res_status.status(), reqwest::StatusCode::OK);
    let status_json: Value = res_status.json().await.expect("Failed to parse status JSON");
    assert_eq!(status_json["symbolCount"], 26);
    assert!(!status_json["activeSnapshotCid"].as_str().unwrap().is_empty());
    let active_cid = status_json["activeSnapshotCid"].as_str().unwrap().to_string();

    // 3. GET /api/symbols returns 26 healthy seeded symbols
    let res_symbols = client.get(format!("{}/api/symbols", base_url)).send().await.expect("Failed to GET /api/symbols");
    assert_eq!(res_symbols.status(), reqwest::StatusCode::OK);
    let symbols_json: Value = res_symbols.json().await.expect("Failed to parse symbols JSON");
    let symbols_arr = symbols_json.as_array().expect("Expected JSON array");
    assert_eq!(symbols_arr.len(), 26);
    
    // Verify first letter 'a' fields
    let symbol_a = &symbols_arr[0];
    assert_eq!(symbol_a["position"], 1);
    assert_eq!(symbol_a["surfaceForm"], "a");
    assert_eq!(symbol_a["canonicalEntityId"], "urn:language-graph:grapheme:nfc:0061");
    assert_eq!(symbol_a["normalization"], "NFC");
    assert_eq!(symbol_a["status"], "Healthy");
    let active_rev_a_cid = symbol_a["activeRevisionCid"].as_str().unwrap().to_string();

    // Verify GET /api/symbols/:entity_id endpoint
    let res_detail = client.get(format!("{}/api/symbols/urn:language-graph:grapheme:nfc:0061", base_url))
        .send().await.expect("Failed to GET details");
    assert_eq!(res_detail.status(), reqwest::StatusCode::OK);
    let detail_json: Value = res_detail.json().await.expect("Failed to parse detail JSON");
    assert_eq!(detail_json["entityId"], "urn:language-graph:grapheme:nfc:0061");
    assert_eq!(detail_json["revisionCid"], active_rev_a_cid);
    assert_eq!(detail_json["surfaceForm"], "a");
    assert_eq!(detail_json["codec"], "dag-cbor");

    // Verify GET /api/snapshots/active endpoint
    let res_snap = client.get(format!("{}/api/snapshots/active", base_url)).send().await.expect("Failed to GET active snapshot");
    assert_eq!(res_snap.status(), reqwest::StatusCode::OK);
    let snap_json: Value = res_snap.json().await.expect("Failed to parse snapshot JSON");
    assert_eq!(snap_json["collectionEntityId"], "urn:language-graph:collection:latin-lowercase-a-z");
    assert_eq!(snap_json["members"].as_array().unwrap().len(), 26);

    // 4. POST /api/resolve returns trace and snapshot CID for valid input
    let res_resolve_ok = client.post(format!("{}/api/resolve", base_url))
        .json(&serde_json::json!({ "text": "banana" }))
        .send().await.expect("Failed to POST resolve");
    assert_eq!(res_resolve_ok.status(), reqwest::StatusCode::OK);
    let resolve_ok_json: Value = res_resolve_ok.json().await.expect("Failed to parse resolve JSON");
    assert_eq!(resolve_ok_json["output"], "banana");
    assert_eq!(resolve_ok_json["collectionSnapshotCid"], active_cid);
    let trace_arr = resolve_ok_json["trace"].as_array().unwrap();
    assert_eq!(trace_arr.len(), 6);
    assert_eq!(trace_arr[0]["inputGrapheme"], "b");
    assert_eq!(trace_arr[0]["status"], "Resolved");
    assert_eq!(trace_arr[5]["inputGrapheme"], "a");
    assert_eq!(trace_arr[5]["status"], "Reused");

    // 5. POST /api/resolve returns structured validation error for invalid input
    let res_resolve_err = client.post(format!("{}/api/resolve", base_url))
        .json(&serde_json::json!({ "text": "banana1" }))
        .send().await.expect("Failed to POST resolve with invalid input");
    assert_eq!(res_resolve_err.status(), reqwest::StatusCode::BAD_REQUEST);
    let resolve_err_json: Value = res_resolve_err.json().await.expect("Failed to parse resolve error JSON");
    assert!(resolve_err_json["error"].as_str().unwrap().contains("Unsupported character or grapheme: '1'"));
}
