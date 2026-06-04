use crate::content::cid::compute_cid;
use crate::content::encoding::to_dag_cbor;
use crate::db::repository::Repository;
use crate::error::Error;
use crate::model::{
    AlphabetSnapshot, GraphemeRevision, ProfileCollectionRef, SnapshotMember, TextProfileSnapshot,
};
use crate::seed::lowercase_latin::COLLECTION_ENTITY_ID as LOW_COL_ID;
use crate::seed::phase2::{
    seed_phase2, DIGITS_COLLECTION_ENTITY_ID, PUNCTUATION_COLLECTION_ENTITY_ID,
    UPPERCASE_COLLECTION_ENTITY_ID, WHITESPACE_COLLECTION_ENTITY_ID,
};
use rusqlite::Connection;
use unicode_normalization::UnicodeNormalization;

pub const PROFILE_2_1_ENTITY_ID: &str = "urn:language-graph:profile:printable-ascii-text";
pub const SUPPLEMENTAL_COLLECTION_ENTITY_ID: &str =
    "urn:language-graph:collection:ascii-supplemental-symbols";

pub fn seed_phase2_1(conn: &mut Connection) -> Result<String, Error> {
    // 1. First seed Phase 2 (which seeds Phase 1 lowercase too, and commits transactions)
    let profile_2_snapshot_cid = seed_phase2(conn)?;

    // 2. Start transaction for Phase 2.1 seeding
    let tx = conn.transaction()?;
    let repo = Repository::new(&tx);

    // Retrieve snapshots of the existing five collections from the active Phase 2 profile snapshot
    let profile_2 = repo.get_profile_snapshot(&profile_2_snapshot_cid)?;
    let lowercase_snapshot_cid = profile_2.collections[0].snapshot_cid.clone();
    let uppercase_snapshot_cid = profile_2.collections[1].snapshot_cid.clone();
    let digits_snapshot_cid = profile_2.collections[2].snapshot_cid.clone();
    let whitespace_snapshot_cid = profile_2.collections[3].snapshot_cid.clone();
    let punctuation_snapshot_cid = profile_2.collections[4].snapshot_cid.clone();

    // 3. Seed 21 supplemental ASCII symbols
    let supplemental_chars = vec![
        (
            '#',
            "Common".to_string(),
            "none".to_string(),
            "NUMBER SIGN".to_string(),
        ),
        (
            '$',
            "Common".to_string(),
            "none".to_string(),
            "DOLLAR SIGN".to_string(),
        ),
        (
            '%',
            "Common".to_string(),
            "none".to_string(),
            "PERCENT SIGN".to_string(),
        ),
        (
            '&',
            "Common".to_string(),
            "none".to_string(),
            "AMPERSAND".to_string(),
        ),
        (
            '*',
            "Common".to_string(),
            "none".to_string(),
            "ASTERISK".to_string(),
        ),
        (
            '+',
            "Common".to_string(),
            "none".to_string(),
            "PLUS SIGN".to_string(),
        ),
        (
            '/',
            "Common".to_string(),
            "none".to_string(),
            "SOLIDUS".to_string(),
        ),
        (
            '<',
            "Common".to_string(),
            "none".to_string(),
            "LESS-THAN SIGN".to_string(),
        ),
        (
            '=',
            "Common".to_string(),
            "none".to_string(),
            "EQUALS SIGN".to_string(),
        ),
        (
            '>',
            "Common".to_string(),
            "none".to_string(),
            "GREATER-THAN SIGN".to_string(),
        ),
        (
            '@',
            "Common".to_string(),
            "none".to_string(),
            "COMMERCIAL AT".to_string(),
        ),
        (
            '[',
            "Common".to_string(),
            "none".to_string(),
            "LEFT SQUARE BRACKET".to_string(),
        ),
        (
            '\\',
            "Common".to_string(),
            "none".to_string(),
            "REVERSE SOLIDUS".to_string(),
        ),
        (
            ']',
            "Common".to_string(),
            "none".to_string(),
            "RIGHT SQUARE BRACKET".to_string(),
        ),
        (
            '^',
            "Common".to_string(),
            "none".to_string(),
            "CIRCUMFLEX ACCENT".to_string(),
        ),
        (
            '_',
            "Common".to_string(),
            "none".to_string(),
            "LOW LINE".to_string(),
        ),
        (
            '`',
            "Common".to_string(),
            "none".to_string(),
            "GRAVE ACCENT".to_string(),
        ),
        (
            '{',
            "Common".to_string(),
            "none".to_string(),
            "LEFT CURLY BRACKET".to_string(),
        ),
        (
            '|',
            "Common".to_string(),
            "none".to_string(),
            "VERTICAL LINE".to_string(),
        ),
        (
            '}',
            "Common".to_string(),
            "none".to_string(),
            "RIGHT CURLY BRACKET".to_string(),
        ),
        (
            '~',
            "Common".to_string(),
            "none".to_string(),
            "TILDE".to_string(),
        ),
    ];

    let supplemental_snapshot_cid = seed_supplemental_collection(
        &repo,
        SUPPLEMENTAL_COLLECTION_ENTITY_ID,
        "ascii-supplemental-symbols",
        "ASCII Supplemental Symbols",
        "ordered-grapheme-collection",
        supplemental_chars,
    )?;

    // 4. Build and seed the Printable ASCII Text Profile snapshot
    let profile_snapshot = TextProfileSnapshot {
        schema: "language-graph/text-profile-snapshot/v1".to_string(),
        profile_entity_id: PROFILE_2_1_ENTITY_ID.to_string(),
        kind: "written-text-profile".to_string(),
        label: "Printable ASCII Text Profile".to_string(),
        collections: vec![
            ProfileCollectionRef {
                position: 1,
                collection_entity_id: LOW_COL_ID.to_string(),
                snapshot_cid: lowercase_snapshot_cid,
            },
            ProfileCollectionRef {
                position: 2,
                collection_entity_id: UPPERCASE_COLLECTION_ENTITY_ID.to_string(),
                snapshot_cid: uppercase_snapshot_cid,
            },
            ProfileCollectionRef {
                position: 3,
                collection_entity_id: DIGITS_COLLECTION_ENTITY_ID.to_string(),
                snapshot_cid: digits_snapshot_cid,
            },
            ProfileCollectionRef {
                position: 4,
                collection_entity_id: WHITESPACE_COLLECTION_ENTITY_ID.to_string(),
                snapshot_cid: whitespace_snapshot_cid,
            },
            ProfileCollectionRef {
                position: 5,
                collection_entity_id: PUNCTUATION_COLLECTION_ENTITY_ID.to_string(),
                snapshot_cid: punctuation_snapshot_cid,
            },
            ProfileCollectionRef {
                position: 6,
                collection_entity_id: SUPPLEMENTAL_COLLECTION_ENTITY_ID.to_string(),
                snapshot_cid: supplemental_snapshot_cid,
            },
        ],
    };

    let profile_bytes = to_dag_cbor(&profile_snapshot)?;
    let profile_cid = compute_cid(&profile_bytes)?;
    let profile_cid_str = profile_cid.to_string();

    if repo.block_exists(&profile_cid_str)? {
        let existing_profile = repo.get_profile_snapshot(&profile_cid_str)?;
        if existing_profile != profile_snapshot {
            return Err(Error::IntegrityError(format!(
                "Seeding conflict: existing profile snapshot for CID {} does not match generated profile snapshot",
                profile_cid_str
            )));
        }
    }

    repo.insert_block(
        &profile_cid_str,
        "dag-cbor",
        "sha2-256",
        "text_profile_snapshot",
        &profile_bytes,
    )?;
    repo.insert_profile(
        PROFILE_2_1_ENTITY_ID,
        "printable-ascii-text",
        "Printable ASCII Text Profile",
    )?;
    repo.insert_profile_snapshot(&profile_cid_str, PROFILE_2_1_ENTITY_ID)?;

    for ref_col in &profile_snapshot.collections {
        repo.insert_profile_snapshot_collection(
            &profile_cid_str,
            ref_col.position,
            &ref_col.collection_entity_id,
            &ref_col.snapshot_cid,
        )?;
    }

    repo.set_active_profile_snapshot(PROFILE_2_1_ENTITY_ID, &profile_cid_str)?;

    tx.commit()?;

    Ok(profile_cid_str)
}

fn seed_supplemental_collection(
    repo: &Repository,
    collection_entity_id: &str,
    canonical_key: &str,
    label: &str,
    kind: &str,
    chars: Vec<(char, String, String, String)>,
) -> Result<String, Error> {
    let mut members = Vec::new();

    for (i, (ch, script, case, _name)) in chars.into_iter().enumerate() {
        let surface_form: String = ch.to_string().nfc().collect();
        let scalar_val = ch as u32;
        let scalar_str = format!("U+{:04X}", scalar_val);
        let hex_id = format!("{:04x}", scalar_val);
        let entity_id = format!("urn:language-graph:grapheme:nfc:{}", hex_id);
        let g_canonical_key = surface_form.clone();
        let g_label = format!("grapheme '{}'", surface_form);

        let rev = GraphemeRevision {
            schema: "language-graph/grapheme-revision/v1".to_string(),
            entity_id: entity_id.clone(),
            kind: "grapheme".to_string(),
            surface_form: surface_form.clone(),
            normalized_form: surface_form.clone(),
            normalization: "NFC".to_string(),
            unicode_scalars: vec![scalar_str],
            script,
            case,
            previous_revision_cid: None,
        };

        let bytes = to_dag_cbor(&rev)?;
        let cid = compute_cid(&bytes)?;
        let cid_str = cid.to_string();

        if repo.block_exists(&cid_str)? {
            let existing_rev = repo.get_grapheme_revision(&cid_str)?;
            if existing_rev != rev {
                return Err(Error::IntegrityError(format!(
                    "Seeding conflict: existing block for CID {} does not match generated grapheme revision for '{}'",
                    cid_str, surface_form
                )));
            }
        }

        repo.insert_block(
            &cid_str,
            "dag-cbor",
            "sha2-256",
            "grapheme_revision",
            &bytes,
        )?;
        repo.insert_entity(&entity_id, "grapheme", &g_canonical_key, &g_label)?;
        repo.set_entity_head(&entity_id, &cid_str)?;

        members.push(SnapshotMember {
            position: (i + 1) as i32,
            entity_id: entity_id.clone(),
            revision_cid: cid_str,
        });
    }

    let snapshot = AlphabetSnapshot {
        schema: "language-graph/collection-snapshot/v1".to_string(),
        collection_entity_id: collection_entity_id.to_string(),
        kind: kind.to_string(),
        label: label.to_string(),
        members,
    };

    let snap_bytes = to_dag_cbor(&snapshot)?;
    let snap_cid = compute_cid(&snap_bytes)?;
    let snap_cid_str = snap_cid.to_string();

    if repo.block_exists(&snap_cid_str)? {
        let existing_snap = repo.get_alphabet_snapshot(&snap_cid_str)?;
        if existing_snap != snapshot {
            return Err(Error::IntegrityError(format!(
                "Seeding conflict: existing snapshot for CID {} does not match generated snapshot",
                snap_cid_str
            )));
        }
    }

    repo.insert_block(
        &snap_cid_str,
        "dag-cbor",
        "sha2-256",
        "collection_snapshot",
        &snap_bytes,
    )?;
    repo.insert_collection(collection_entity_id, canonical_key, label)?;
    repo.insert_snapshot(&snap_cid_str, collection_entity_id)?;

    for member in &snapshot.members {
        repo.insert_snapshot_member(
            &snap_cid_str,
            member.position,
            &member.entity_id,
            &member.revision_cid,
        )?;
    }

    repo.set_active_snapshot(collection_entity_id, &snap_cid_str)?;

    Ok(snap_cid_str)
}
