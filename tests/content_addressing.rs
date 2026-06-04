use language_graph_engine::content::cid::compute_cid;
use language_graph_engine::content::encoding::{to_dag_cbor, from_dag_cbor};
use language_graph_engine::model::{GraphemeRevision, AlphabetSnapshot, SnapshotMember};
use rusqlite::Connection;

#[test]
fn test_mandatory_foundational_proof() {
    // 1. Build the immutable revision object for symbol `a`
    let rev_a = GraphemeRevision {
        schema: "language-graph/grapheme-revision/v1".to_string(),
        entity_id: "urn:language-graph:grapheme:nfc:0061".to_string(),
        kind: "grapheme".to_string(),
        surface_form: "a".to_string(),
        normalized_form: "a".to_string(),
        normalization: "NFC".to_string(),
        unicode_scalars: vec!["U+0061".to_string()],
        script: "Latn".to_string(),
        case: "lowercase".to_string(),
        previous_revision_cid: None,
    };

    // 2. Encode it using the selected canonical DAG-CBOR implementation
    let bytes_a1 = to_dag_cbor(&rev_a).expect("Failed to encode rev_a to DAG-CBOR");

    // 3. Generate a CIDv1 using SHA2-256
    let cid_a1 = compute_cid(&bytes_a1).expect("Failed to compute CID");

    // 4. Encode the same logical object again and confirm the bytes are identical
    let bytes_a2 = to_dag_cbor(&rev_a).expect("Failed to encode rev_a to DAG-CBOR 2nd time");
    assert_eq!(bytes_a1, bytes_a2, "Encoded bytes are not identical!");

    // 5. Confirm the CID is identical
    let cid_a2 = compute_cid(&bytes_a2).expect("Failed to compute CID 2nd time");
    assert_eq!(cid_a1, cid_a2, "Computed CIDs are not identical!");

    // 6. Store the block bytes and CID in a temporary SQLite database
    let conn = Connection::open_in_memory().expect("Failed to open in-memory SQLite");
    conn.execute(
        "CREATE TABLE immutable_blocks (
            cid TEXT PRIMARY KEY,
            codec TEXT NOT NULL,
            multihash_algorithm TEXT NOT NULL,
            block_kind TEXT NOT NULL,
            bytes BLOB NOT NULL,
            stored_at TEXT NOT NULL
        )",
        [],
    ).expect("Failed to create temporary table");

    let cid_str = cid_a1.to_string();
    conn.execute(
        "INSERT INTO immutable_blocks (cid, codec, multihash_algorithm, block_kind, bytes, stored_at)
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
        (
            &cid_str,
            "dag-cbor",
            "sha2-256",
            "grapheme_revision",
            &bytes_a1,
        ),
    ).expect("Failed to insert block into SQLite");

    // 7. Retrieve the bytes
    let retrieved_bytes: Vec<u8> = conn.query_row(
        "SELECT bytes FROM immutable_blocks WHERE cid = ?1",
        [&cid_str],
        |row| row.get(0),
    ).expect("Failed to query block bytes");

    assert_eq!(bytes_a1, retrieved_bytes, "Retrieved bytes differ from original!");

    // 8. Decode the bytes into the Rust model
    let decoded_rev_a: GraphemeRevision = from_dag_cbor(&retrieved_bytes)
        .expect("Failed to decode DAG-CBOR back to GraphemeRevision");
    
    assert_eq!(rev_a, decoded_rev_a, "Decoded object does not match original!");

    // 9. Re-encode and recompute the CID
    let re_encoded_bytes = to_dag_cbor(&decoded_rev_a).expect("Failed to re-encode");
    let re_computed_cid = compute_cid(&re_encoded_bytes).expect("Failed to re-compute CID");

    // 10. Confirm it still matches exactly
    assert_eq!(cid_a1, re_computed_cid, "Recomputed CID does not match original CID!");

    // 11. Build the initial full 26-member alphabet snapshot twice independently and confirm the same snapshot CID is produced
    let build_snapshot = || {
        let mut members = Vec::new();
        for i in 0..26 {
            let ch = (b'a' + i) as char;
            let entity_id = format!("urn:language-graph:grapheme:nfc:{:04x}", ch as u32);
            // Simulating revision CIDs
            let rev_cid = format!("bagybeiertyuiopasdfghjklzxcvbnm{}", i);
            members.push(SnapshotMember {
                position: (i + 1) as i32,
                entity_id,
                revision_cid: rev_cid,
            });
        }

        AlphabetSnapshot {
            schema: "language-graph/collection-snapshot/v1".to_string(),
            collection_entity_id: "urn:language-graph:collection:latin-lowercase-a-z".to_string(),
            kind: "ordered-grapheme-collection".to_string(),
            label: "Latin lowercase alphabet a-z".to_string(),
            members,
        }
    };

    let snap1 = build_snapshot();
    let snap2 = build_snapshot();

    let bytes_snap1 = to_dag_cbor(&snap1).expect("Failed to encode snapshot 1");
    let bytes_snap2 = to_dag_cbor(&snap2).expect("Failed to encode snapshot 2");
    assert_eq!(bytes_snap1, bytes_snap2, "Snapshot bytes are not identical!");

    let cid_snap1 = compute_cid(&bytes_snap1).expect("Failed to compute snapshot 1 CID");
    let cid_snap2 = compute_cid(&bytes_snap2).expect("Failed to compute snapshot 2 CID");
    assert_eq!(cid_snap1, cid_snap2, "Snapshot CIDs are not identical!");

    println!("Deterministic content addressing proof succeeded! CID: {}", cid_a1);
}
