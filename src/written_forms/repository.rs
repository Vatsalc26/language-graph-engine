use crate::content::cid::compute_cid;
use crate::content::encoding::{from_dag_cbor, to_dag_cbor};
use crate::error::Error;
use crate::model::{WrittenFormComponent, WrittenFormRevision};
use crate::resolver::text::TextResolver;
use crate::written_forms::composer::preview_written_form;
use rusqlite::{params, Connection, OptionalExtension};
use unicode_normalization::UnicodeNormalization;

pub const STORE_ENTITY_ID: &str = "urn:language-graph:store:english-natural-language-written-forms";

#[derive(serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub surface_form: String,
    pub entity_id: String,
    pub revision_cid: String,
    pub composition_profile_snapshot_cid: String,
    pub components: Vec<WrittenFormComponent>,
    pub store_membership: String,
    pub status: String, // "Created" or "Already Stored"
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StoredWrittenFormSummary {
    pub surface_form: String,
    pub entity_id: String,
    pub revision_cid: String,
    pub component_count: i32,
    pub stored_status: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WrittenFormDetails {
    pub surface_form: String,
    pub entity_id: String,
    pub revision_cid: String,
    pub composition_profile_snapshot_cid: String,
    pub components: Vec<WrittenFormComponent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attestations: Option<Vec<String>>,
}

pub fn insert_or_reuse_written_form_within_transaction(
    tx: &rusqlite::Transaction,
    resolver: &TextResolver,
    surface_form: &str,
    normalized: &str,
    components: &[WrittenFormComponent],
) -> Result<(String, String, bool), Error> {
    let entity_id = crate::written_forms::composer::derive_entity_id(normalized);
    let profile_cid = &resolver.active_snapshot_cid;

    // Check if the entity already has a head in the database
    let existing_head: Option<String> = tx
        .query_row(
            "SELECT revision_cid FROM entity_heads WHERE entity_id = ?1",
            [&entity_id],
            |row| row.get(0),
        )
        .optional()?;

    if let Some(head_cid) = existing_head {
        // Already stored, check if the composition matches exactly (idempotency)
        let block_bytes: Option<Vec<u8>> = tx
            .query_row(
                "SELECT bytes FROM immutable_blocks WHERE cid = ?1",
                [&head_cid],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(bytes) = block_bytes {
            // Verify integrity
            let computed_cid = compute_cid(&bytes)?;
            if computed_cid.to_string() != head_cid {
                return Err(Error::IntegrityError(format!(
                    "Integrity check failed: head CID '{}' does not match block bytes hash '{}'",
                    head_cid, computed_cid
                )));
            }

            let existing_rev: WrittenFormRevision = from_dag_cbor(&bytes)?;
            if existing_rev.components == components
                && existing_rev.composition_profile_snapshot_cid == *profile_cid
            {
                // Same components and profile: idempotent return
                return Ok((entity_id, head_cid, false));
            } else {
                // Conflict
                return Err(Error::IntegrityError(format!(
                    "Conflicting composition for written form '{}' (entity {}). Existing revision is {}, but new composition has different components or profile.",
                    surface_form, entity_id, head_cid
                )));
            }
        } else {
            return Err(Error::NotFoundError(format!(
                "Entity head revision block {} not found in database",
                head_cid
            )));
        }
    }

    // Create new WrittenFormRevision
    let revision = WrittenFormRevision {
        schema: "language-graph/written-form-revision/v1".to_string(),
        entity_id: entity_id.clone(),
        kind: "written-form".to_string(),
        form_class: "natural-language-written-form".to_string(),
        surface_form: surface_form.to_string(),
        normalized_form: normalized.to_string(),
        normalization: "NFC".to_string(),
        composition_profile_snapshot_cid: profile_cid.clone(),
        components: components.to_vec(),
        previous_revision_cid: None,
    };

    // Encode as CBOR and compute CID
    let bytes = to_dag_cbor(&revision)?;
    let cid = compute_cid(&bytes)?;
    let cid_str = cid.to_string();

    // Store in database
    // Insert block (IGNORE if identical block already exists)
    tx.execute(
        "INSERT OR IGNORE INTO immutable_blocks (cid, codec, multihash_algorithm, block_kind, bytes, stored_at)
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
        params![cid_str, "dag-cbor", "sha2-256", "written_form_revision", bytes],
    )?;

    // Insert into entities table
    let canonical_key = format!("written-form:{}", surface_form);
    let label = format!("written form '{}'", surface_form);

    let existing_entity: Option<(String, String)> = tx
        .query_row(
            "SELECT canonical_key, label FROM entities WHERE entity_id = ?1",
            [&entity_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;

    if let Some((existing_key, existing_label)) = existing_entity {
        if existing_key != canonical_key || existing_label != label {
            return Err(Error::IntegrityError(format!(
                "Conflicting entity metadata for {}: existing_key='{}', new_key='{}'",
                entity_id, existing_key, canonical_key
            )));
        }
    } else {
        tx.execute(
            "INSERT INTO entities (entity_id, kind, canonical_key, label, created_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            params![entity_id, "written-form", canonical_key, label],
        )?;
    }

    // Insert into written_forms table
    tx.execute(
        "INSERT INTO written_forms (entity_id, surface_form, normalized_surface_form, form_class, created_at)
         VALUES (?1, ?2, ?3, ?4, datetime('now'))",
        params![entity_id, surface_form, normalized, "natural-language-written-form"],
    )?;

    // Set entity head in entity_heads
    tx.execute(
        "INSERT INTO entity_heads (entity_id, revision_cid, updated_at)
         VALUES (?1, ?2, datetime('now'))",
        params![entity_id, cid_str],
    )?;

    // Insert component relations in written_form_components
    for comp in components {
        tx.execute(
            "INSERT INTO written_form_components (written_form_revision_cid, position, symbol_entity_id, symbol_revision_cid, surface_form)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                cid_str,
                comp.position,
                comp.symbol_entity_id,
                comp.symbol_revision_cid,
                comp.surface_form
            ],
        )?;
    }

    Ok((entity_id, cid_str, true))
}

pub fn save_written_form(
    resolver: &TextResolver,
    conn: &mut Connection,
    surface_form: &str,
) -> Result<SaveResult, Error> {
    // 1. Run preview logic to validate and compose
    let preview = preview_written_form(resolver, conn, surface_form)?;
    if !preview.is_eligible {
        return Err(Error::ValidationError(
            preview
                .validation_message
                .unwrap_or_else(|| "Not eligible".to_string()),
        ));
    }

    let profile_cid = preview.active_profile_snapshot_cid.unwrap();
    let components = preview.components.unwrap();
    let normalized = preview.normalized_form;

    // Start transaction for writing
    let tx = conn.transaction()?;

    let (eid, cid_str, is_new) = insert_or_reuse_written_form_within_transaction(
        &tx,
        resolver,
        surface_form,
        &normalized,
        &components,
    )?;

    // Add membership to English natural-language written forms store
    tx.execute(
        "INSERT OR IGNORE INTO written_form_store_members (store_entity_id, written_form_entity_id, admitted_revision_cid, added_at, status)
         VALUES (?1, ?2, ?3, datetime('now'), ?4)",
        params![STORE_ENTITY_ID, eid, cid_str, "active"],
    )?;

    // Add attestation for manual entry
    tx.execute(
        "INSERT OR IGNORE INTO written_form_attestations (source_entity_id, written_form_entity_id, first_import_id, latest_import_id, source_surface_form, attestation_status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            "urn:language-graph:lexicon-source:manual",
            eid,
            "manual",
            "manual",
            surface_form,
            "active"
        ],
    )?;

    tx.commit()?;

    let status = if is_new {
        "Created".to_string()
    } else {
        "Already Stored".to_string()
    };

    Ok(SaveResult {
        surface_form: surface_form.to_string(),
        entity_id: eid,
        revision_cid: cid_str,
        composition_profile_snapshot_cid: profile_cid,
        components,
        store_membership: STORE_ENTITY_ID.to_string(),
        status,
    })
}

pub fn find_written_form_exact(
    conn: &rusqlite::Connection,
    surface_form: &str,
) -> Result<Option<StoredWrittenFormSummary>, Error> {
    let normalized: String = surface_form.nfc().collect();

    let result: Option<(String, String, String, i64)> = conn
        .query_row(
            "SELECT wf.entity_id, wf.surface_form, eh.revision_cid, 
                    (SELECT COUNT(*) FROM written_form_components wfc WHERE wfc.written_form_revision_cid = eh.revision_cid)
             FROM written_forms wf
             JOIN entity_heads eh ON wf.entity_id = eh.entity_id
             WHERE wf.normalized_surface_form = ?1 COLLATE BINARY",
            [&normalized],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;

    if let Some((entity_id, exact_surface, revision_cid, comp_count)) = result {
        if exact_surface != surface_form {
            return Ok(None);
        }
        Ok(Some(StoredWrittenFormSummary {
            surface_form: exact_surface,
            entity_id,
            revision_cid,
            component_count: comp_count as i32,
            stored_status: "Stored".to_string(),
        }))
    } else {
        Ok(None)
    }
}

pub fn list_written_forms(
    conn: &rusqlite::Connection,
    store_entity_id: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<StoredWrittenFormSummary>, Error> {
    let mut stmt = conn.prepare(
        "SELECT wf.entity_id, wf.surface_form, eh.revision_cid,
                (SELECT COUNT(*) FROM written_form_components wfc WHERE wfc.written_form_revision_cid = eh.revision_cid)
         FROM written_form_store_members wfsm
         JOIN written_forms wf ON wfsm.written_form_entity_id = wf.entity_id
         JOIN entity_heads eh ON wf.entity_id = eh.entity_id
         WHERE wfsm.store_entity_id = ?1 AND wfsm.status = 'active'
         ORDER BY wf.normalized_surface_form COLLATE BINARY ASC
         LIMIT ?2 OFFSET ?3",
    )?;

    let rows = stmt.query_map(params![store_entity_id, limit, offset], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })?;

    let mut results = Vec::new();
    for row in rows {
        let (entity_id, surface_form, revision_cid, comp_count) = row?;
        results.push(StoredWrittenFormSummary {
            surface_form,
            entity_id,
            revision_cid,
            component_count: comp_count as i32,
            stored_status: "Stored".to_string(),
        });
    }

    Ok(results)
}

pub fn get_written_form_details(
    conn: &rusqlite::Connection,
    id_or_surface: &str,
) -> Result<Option<WrittenFormDetails>, Error> {
    let result: Option<(String, String, String)> =
        if id_or_surface.starts_with("urn:language-graph:") {
            conn.query_row(
                "SELECT wf.entity_id, wf.surface_form, eh.revision_cid
             FROM written_forms wf
             JOIN entity_heads eh ON wf.entity_id = eh.entity_id
             WHERE wf.entity_id = ?1",
                [id_or_surface],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
        } else {
            let normalized: String = id_or_surface.nfc().collect();
            conn.query_row(
                "SELECT wf.entity_id, wf.surface_form, eh.revision_cid
             FROM written_forms wf
             JOIN entity_heads eh ON wf.entity_id = eh.entity_id
             WHERE wf.normalized_surface_form = ?1 COLLATE BINARY",
                [&normalized],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
        };

    let (entity_id, surface_form, revision_cid) = match result {
        Some(res) => res,
        None => return Ok(None),
    };

    if !id_or_surface.starts_with("urn:language-graph:") && surface_form != id_or_surface {
        return Ok(None);
    }

    let mut stmt = conn.prepare(
        "SELECT position, symbol_entity_id, symbol_revision_cid, surface_form
         FROM written_form_components
         WHERE written_form_revision_cid = ?1
         ORDER BY position ASC",
    )?;

    let rows = stmt.query_map([&revision_cid], |row| {
        Ok(WrittenFormComponent {
            position: row.get(0)?,
            symbol_entity_id: row.get(1)?,
            symbol_revision_cid: row.get(2)?,
            surface_form: row.get(3)?,
        })
    })?;

    let mut components = Vec::new();
    for comp in rows {
        components.push(comp?);
    }

    let block_bytes = conn.query_row(
        "SELECT bytes FROM immutable_blocks WHERE cid = ?1",
        [&revision_cid],
        |row| row.get::<_, Vec<u8>>(0),
    )?;

    let revision_data: WrittenFormRevision = from_dag_cbor(&block_bytes)?;

    // Query attestations from the database
    let mut stmt_att = conn.prepare(
        "SELECT ls.label 
         FROM written_form_attestations wfa
         JOIN lexicon_sources ls ON wfa.source_entity_id = ls.source_entity_id
         WHERE wfa.written_form_entity_id = ?1 AND wfa.attestation_status = 'active'
         ORDER BY ls.label ASC",
    )?;
    let att_rows = stmt_att.query_map([&entity_id], |row| row.get::<_, String>(0))?;
    let mut attestations = Vec::new();
    for att in att_rows {
        attestations.push(att?);
    }
    let attestations_opt = if attestations.is_empty() {
        None
    } else {
        Some(attestations)
    };

    Ok(Some(WrittenFormDetails {
        surface_form,
        entity_id,
        revision_cid,
        composition_profile_snapshot_cid: revision_data.composition_profile_snapshot_cid,
        components,
        attestations: attestations_opt,
    }))
}
