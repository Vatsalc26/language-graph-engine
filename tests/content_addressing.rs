use language_graph_engine::content::cid::compute_cid;
use language_graph_engine::content::encoding::{from_dag_cbor, to_dag_cbor};
use language_graph_engine::model::{AlphabetSnapshot, GraphemeRevision, SnapshotMember};
use rusqlite::Connection;
use unicode_normalization::UnicodeNormalization;

// Golden test vector constants
pub const GOLDEN_A_CID: &str = "bafyreigzc6usxy4ufmz43vpotqo54cqrdqwtzgebsxwhyybmkgphbfwq5a";
pub const GOLDEN_Z_CID: &str = "bafyreiamiyi7szcus6c67balumi6ejdvby5jiad5yr375sveyufkkb63dm";
pub const GOLDEN_SNAPSHOT_CID: &str = "bafyreib4ivpoazb5skkr7yvfelvoowz6sxxncdsjewvxawyedm5tikeshm";

fn make_revision(ch: char) -> GraphemeRevision {
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
        script: "Latn".to_string(),
        case: "lowercase".to_string(),
        previous_revision_cid: None,
    }
}

#[test]
fn test_deterministic_symbol_revision_encoding() {
    let rev_a1 = make_revision('a');
    let rev_a2 = make_revision('a');

    let bytes1 = to_dag_cbor(&rev_a1).expect("Encode revision 1");
    let bytes2 = to_dag_cbor(&rev_a2).expect("Encode revision 2");
    assert_eq!(bytes1, bytes2, "Encoded DAG-CBOR bytes must be identical");

    let cid1 = compute_cid(&bytes1).expect("Compute CID 1");
    let cid2 = compute_cid(&bytes2).expect("Compute CID 2");
    assert_eq!(cid1, cid2, "Generated CIDs must be identical");

    // Assert CIDv1 / DAG-CBOR / SHA2-256 properties
    assert_eq!(cid1.version(), cid::Version::V1, "Must be CIDv1");
    assert_eq!(cid1.codec(), 0x71, "Codec must be 0x71 (dag-cbor)");
    assert_eq!(
        cid1.hash().code(),
        0x12,
        "Hash algorithm must be 0x12 (sha2-256)"
    );
}

#[test]
fn test_storage_round_trip_preserves_cid() {
    let rev_a = make_revision('a');
    let bytes = to_dag_cbor(&rev_a).expect("Encode");
    let cid = compute_cid(&bytes).expect("Compute CID");
    let cid_str = cid.to_string();

    let conn = Connection::open_in_memory().expect("In-memory database");
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
    )
    .expect("Create table");

    conn.execute(
        "INSERT INTO immutable_blocks (cid, codec, multihash_algorithm, block_kind, bytes, stored_at)
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
        (&cid_str, "dag-cbor", "sha2-256", "grapheme_revision", &bytes),
    ).expect("Insert block");

    let retrieved_bytes: Vec<u8> = conn
        .query_row(
            "SELECT bytes FROM immutable_blocks WHERE cid = ?1",
            [&cid_str],
            |row| row.get(0),
        )
        .expect("Retrieve bytes");

    assert_eq!(
        bytes, retrieved_bytes,
        "Retrieved bytes must match original"
    );

    let decoded: GraphemeRevision = from_dag_cbor(&retrieved_bytes).expect("Decode");
    assert_eq!(rev_a, decoded, "Decoded object must match original");

    let re_encoded = to_dag_cbor(&decoded).expect("Re-encode");
    let re_cid = compute_cid(&re_encoded).expect("Recompute CID");
    assert_eq!(cid, re_cid, "Round-trip CID must remain identical");
}

#[test]
fn test_complete_alphabet_snapshot_determinism() {
    let build_snapshot = || {
        let mut members = Vec::new();
        for i in 0..26 {
            let ch = (b'a' + i) as char;
            let rev = make_revision(ch);
            let bytes = to_dag_cbor(&rev).expect("Encode letter");
            let cid = compute_cid(&bytes).expect("Compute letter CID");
            let hex_id = format!("{:04x}", ch as u32);
            let entity_id = format!("urn:language-graph:grapheme:nfc:{}", hex_id);

            members.push(SnapshotMember {
                position: (i + 1) as i32,
                entity_id,
                revision_cid: cid.to_string(),
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

    let bytes1 = to_dag_cbor(&snap1).expect("Encode snap 1");
    let bytes2 = to_dag_cbor(&snap2).expect("Encode snap 2");
    assert_eq!(bytes1, bytes2, "Snapshot bytes must be identical");

    let cid1 = compute_cid(&bytes1).expect("Compute snap CID 1");
    let cid2 = compute_cid(&bytes2).expect("Compute snap CID 2");
    assert_eq!(cid1, cid2, "Snapshot CIDs must be identical");

    // Verify ordering and members
    assert_eq!(snap1.members.len(), 26, "Must contain 26 members");
    for (i, member) in snap1.members.iter().enumerate() {
        assert_eq!(
            member.position,
            (i + 1) as i32,
            "Positions must be 1-based ordered sequential"
        );
        let ch = (b'a' + i as u8) as char;
        assert!(
            member.entity_id.ends_style_check(ch),
            "Member URN must match letter: {:?}",
            member.entity_id
        );
    }

    assert_eq!(cid1.version(), cid::Version::V1, "Must be CIDv1");
    assert_eq!(cid1.codec(), 0x71, "Codec must be 0x71 (dag-cbor)");
    assert_eq!(cid1.hash().code(), 0x12, "Hash must be 0x12 (sha2-256)");
}

trait EndsStyleCheck {
    fn ends_style_check(&self, ch: char) -> bool;
}

impl EndsStyleCheck for String {
    fn ends_style_check(&self, ch: char) -> bool {
        let hex = format!("{:04x}", ch as u32);
        self.ends_with(&hex)
    }
}

#[test]
fn test_semantic_modification_changes_cid() {
    let mut rev = make_revision('a');
    let bytes_orig = to_dag_cbor(&rev).expect("Encode");
    let cid_orig = compute_cid(&bytes_orig).expect("Compute CID");

    // Change surface form
    rev.surface_form = "x".to_string();
    let bytes_mod1 = to_dag_cbor(&rev).expect("Encode mod 1");
    let cid_mod1 = compute_cid(&bytes_mod1).expect("Compute mod 1 CID");
    assert_ne!(cid_orig, cid_mod1, "Changing surface form must change CID");

    // Reset and change normalized form
    let mut rev = make_revision('a');
    rev.normalized_form = "x".to_string();
    let bytes_mod2 = to_dag_cbor(&rev).expect("Encode mod 2");
    let cid_mod2 = compute_cid(&bytes_mod2).expect("Compute mod 2 CID");
    assert_ne!(
        cid_orig, cid_mod2,
        "Changing normalized form must change CID"
    );
}

#[test]
fn test_operational_metadata_does_not_affect_identity() {
    let rev_a = make_revision('a');
    let bytes = to_dag_cbor(&rev_a).expect("Encode");
    let cid = compute_cid(&bytes).expect("Compute CID");
    let cid_str = cid.to_string();

    let conn = Connection::open_in_memory().expect("In-memory SQLite");
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
    )
    .expect("Create table");

    // Insert block with database stored_at set to '2026-06-04 12:00:00'
    conn.execute(
        "INSERT INTO immutable_blocks (cid, codec, multihash_algorithm, block_kind, bytes, stored_at)
         VALUES (?1, ?2, ?3, ?4, ?5, '2026-06-04 12:00:00')",
        (&cid_str, "dag-cbor", "sha2-256", "grapheme_revision", &bytes),
    ).expect("Insert block 1");

    // Assert block can be retrieved and hashes to identical CID
    let bytes_retrieved: Vec<u8> = conn
        .query_row(
            "SELECT bytes FROM immutable_blocks WHERE cid = ?1",
            [&cid_str],
            |row| row.get(0),
        )
        .expect("Query 1");
    let cid_retrieved = compute_cid(&bytes_retrieved).expect("Compute CID retrieved");
    assert_eq!(cid, cid_retrieved);

    // Verify that the struct itself contains no storage timestamp fields
    // (verified via compile-time GraphemeRevision definition check - it lacks fields for stored_at or database ID)
}

#[test]
fn test_golden_vector_assertions() {
    // Assert letter 'a' Golden CID
    let rev_a = make_revision('a');
    let bytes_a = to_dag_cbor(&rev_a).expect("Encode a");
    let cid_a = compute_cid(&bytes_a).expect("Compute a CID");
    assert_eq!(
        cid_a.to_string(),
        GOLDEN_A_CID,
        "Golden CID for 'a' has changed!"
    );

    // Assert letter 'z' Golden CID
    let rev_z = make_revision('z');
    let bytes_z = to_dag_cbor(&rev_z).expect("Encode z");
    let cid_z = compute_cid(&bytes_z).expect("Compute z CID");
    assert_eq!(
        cid_z.to_string(),
        GOLDEN_Z_CID,
        "Golden CID for 'z' has changed!"
    );

    // Assert Snapshot Golden CID
    let mut members = Vec::new();
    for i in 0..26 {
        let ch = (b'a' + i) as char;
        let rev = make_revision(ch);
        let bytes = to_dag_cbor(&rev).expect("Encode letter");
        let cid = compute_cid(&bytes).expect("Compute letter CID");
        let hex_id = format!("{:04x}", ch as u32);
        let entity_id = format!("urn:language-graph:grapheme:nfc:{}", hex_id);

        members.push(SnapshotMember {
            position: (i + 1) as i32,
            entity_id,
            revision_cid: cid.to_string(),
        });
    }

    let snap = AlphabetSnapshot {
        schema: "language-graph/collection-snapshot/v1".to_string(),
        collection_entity_id: "urn:language-graph:collection:latin-lowercase-a-z".to_string(),
        kind: "ordered-grapheme-collection".to_string(),
        label: "Latin lowercase alphabet a-z".to_string(),
        members,
    };

    let bytes_snap = to_dag_cbor(&snap).expect("Encode snapshot");
    let cid_snap = compute_cid(&bytes_snap).expect("Compute snapshot CID");
    assert_eq!(
        cid_snap.to_string(),
        GOLDEN_SNAPSHOT_CID,
        "Golden CID for snapshot has changed!"
    );
}
