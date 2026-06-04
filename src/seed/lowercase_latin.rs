use crate::content::cid::compute_cid;
use crate::content::encoding::to_dag_cbor;
use crate::db::repository::Repository;
use crate::error::Error;
use crate::model::{AlphabetSnapshot, GraphemeRevision, SnapshotMember};
use rusqlite::Connection;
use unicode_normalization::UnicodeNormalization;

pub const COLLECTION_ENTITY_ID: &str = "urn:language-graph:collection:latin-lowercase-a-z";

pub fn seed_lowercase_latin(conn: &mut Connection) -> Result<String, Error> {
    // Run everything in a transaction to guarantee atomic consistency
    let tx = conn.transaction()?;

    let repo = Repository::new(&tx);

    // 1. Seed lowercase letters 'a' through 'z'
    let mut members = Vec::new();

    for i in 0..26 {
        let ch = (b'a' + i) as char;
        // Normalize surface form to NFC
        let surface_form: String = ch.to_string().nfc().collect();

        // Get unicode scalar code point
        let scalar_val = ch as u32;
        let scalar_str = format!("U+{:04X}", scalar_val);
        let hex_id = format!("{:04x}", scalar_val);

        let entity_id = format!("urn:language-graph:grapheme:nfc:{}", hex_id);
        let canonical_key = surface_form.clone();
        let label = format!("grapheme '{}'", surface_form);

        // Build the symbol revision object
        let rev = GraphemeRevision {
            schema: "language-graph/grapheme-revision/v1".to_string(),
            entity_id: entity_id.clone(),
            kind: "grapheme".to_string(),
            surface_form: surface_form.clone(),
            normalized_form: surface_form.clone(),
            normalization: "NFC".to_string(),
            unicode_scalars: vec![scalar_str],
            script: "Latn".to_string(),
            case: "lowercase".to_string(),
            previous_revision_cid: None,
        };

        // Encode as DAG-CBOR
        let bytes = to_dag_cbor(&rev)?;

        // Compute CIDv1
        let cid = compute_cid(&bytes)?;
        let cid_str = cid.to_string();

        // Check if block exists
        if repo.block_exists(&cid_str)? {
            // Verify it decodes to same object
            let existing_rev = repo.get_grapheme_revision(&cid_str)?;
            if existing_rev != rev {
                return Err(Error::IntegrityError(format!(
                    "Seeding conflict: existing block for CID {} does not match generated grapheme revision for '{}'",
                    cid_str, surface_form
                )));
            }
        }

        // Store block, entity, head
        repo.insert_block(
            &cid_str,
            "dag-cbor",
            "sha2-256",
            "grapheme_revision",
            &bytes,
        )?;
        repo.insert_entity(&entity_id, "grapheme", &canonical_key, &label)?;
        repo.set_entity_head(&entity_id, &cid_str)?;

        // Add to snapshot members list
        members.push(SnapshotMember {
            position: (i + 1) as i32,
            entity_id: entity_id.clone(),
            revision_cid: cid_str,
        });
    }

    // 2. Build the collection snapshot
    let snapshot = AlphabetSnapshot {
        schema: "language-graph/collection-snapshot/v1".to_string(),
        collection_entity_id: COLLECTION_ENTITY_ID.to_string(),
        kind: "ordered-grapheme-collection".to_string(),
        label: "Latin lowercase alphabet a-z".to_string(),
        members,
    };

    // Encode snapshot as DAG-CBOR
    let snap_bytes = to_dag_cbor(&snapshot)?;
    let snap_cid = compute_cid(&snap_bytes)?;
    let snap_cid_str = snap_cid.to_string();

    // Verify snapshot if it exists
    if repo.block_exists(&snap_cid_str)? {
        let existing_snap = repo.get_alphabet_snapshot(&snap_cid_str)?;
        if existing_snap != snapshot {
            return Err(Error::IntegrityError(format!(
                "Seeding conflict: existing snapshot for CID {} does not match generated alphabet snapshot",
                snap_cid_str
            )));
        }
    }

    // Store snapshot block, collection, and snapshots mappings
    repo.insert_block(
        &snap_cid_str,
        "dag-cbor",
        "sha2-256",
        "collection_snapshot",
        &snap_bytes,
    )?;
    repo.insert_collection(
        COLLECTION_ENTITY_ID,
        "latin-lowercase-a-z",
        "Latin lowercase alphabet a-z",
    )?;
    repo.insert_snapshot(&snap_cid_str, COLLECTION_ENTITY_ID)?;

    // Store searchable projections of snapshot members
    for member in &snapshot.members {
        repo.insert_snapshot_member(
            &snap_cid_str,
            member.position,
            &member.entity_id,
            &member.revision_cid,
        )?;
    }

    // Mark that snapshot as the active published snapshot
    repo.set_active_snapshot(COLLECTION_ENTITY_ID, &snap_cid_str)?;

    tx.commit()?;

    Ok(snap_cid_str)
}
