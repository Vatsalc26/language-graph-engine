use crate::content::cid::compute_cid;
use crate::content::encoding::to_dag_cbor;
use crate::error::Error;
use crate::lexicon_import::report::LexiconImportBatchResult;
use crate::model::WrittenFormComponent;
use crate::resolver::text::TextResolver;
use crate::written_forms::publisher::publish_store_snapshot;
use crate::written_forms::repository::{
    insert_or_reuse_written_form_within_transaction, STORE_ENTITY_ID,
};
use multihash_codetable::{Code, MultihashDigest};
use rusqlite::{params, Connection};
use std::time::SystemTime;
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LexiconImportManifest {
    pub schema: String,
    pub source_entity_id: String,
    pub source_release_tag: String,
    pub source_file_name: String,
    pub source_file_sha256: String,
    pub source_entry_count: i64,
    pub admission_policy: String,
    pub accepted_new_count: i64,
    pub accepted_reused_count: i64,
    pub deferred_count: i64,
    pub rejected_or_malformed_count: i64,
    pub resulting_store_snapshot_cid: String,
}

pub fn analyze_esdb_file(
    conn: &Connection,
    content: &[u8],
    expected_count: Option<usize>,
    expected_sha256: Option<&str>,
) -> Result<LexiconImportBatchResult, Error> {
    let hash = Code::Sha2_256.digest(content);
    let digest: String = hash.digest().iter().map(|b| format!("{:02x}", b)).collect();

    if let Some(expected_hash) = expected_sha256 {
        if digest != expected_hash {
            return Err(Error::ValidationError(format!(
                "Expected file SHA2-256 digest to be '{}', but got '{}'.",
                expected_hash, digest
            )));
        }
    }

    let text = std::str::from_utf8(content)
        .map_err(|e| Error::ValidationError(format!("Invalid UTF-8 source file: {:?}", e)))?;

    let lines: Vec<&str> = text.lines().collect();
    let line_count = lines.len();

    if let Some(expected) = expected_count {
        if line_count != expected {
            return Err(Error::ValidationError(format!(
                "Expected exactly {} lines in the wordlist file, but found {}.",
                expected, line_count
            )));
        }
    }

    let mut eligible_new = 0;
    let mut eligible_reused = 0;
    let mut deferred = 0;
    let mut rejected = 0;

    let mut stmt = conn.prepare_cached(
        "SELECT COUNT(*) FROM written_forms WHERE normalized_surface_form = ?1 COLLATE BINARY",
    )?;

    for line in &lines {
        let classification = crate::lexicon_import::esdb::classify_word(line);
        match classification {
            crate::lexicon_import::esdb::Classification::Eligible => {
                let normalized: String = line.nfc().collect();
                let count: i64 = stmt.query_row([&normalized], |row| row.get(0))?;
                if count > 0 {
                    eligible_reused += 1;
                } else {
                    eligible_new += 1;
                }
            }
            crate::lexicon_import::esdb::Classification::Deferred { reason_code, .. } => {
                if reason_code == "malformed_or_empty" {
                    rejected += 1;
                } else {
                    deferred += 1;
                }
            }
        }
    }

    Ok(LexiconImportBatchResult {
        source: "ESDB English (US) rel-2026.02.25 Default Wordlist".to_string(),
        entries_read: line_count,
        eligible_new_words: eligible_new,
        eligible_existing_words_to_reuse: eligible_reused,
        deferred_entries: deferred,
        rejected_or_malformed: rejected,
        source_sha256_digest: digest,
        snapshot_cid: None,
        manifest_cid: None,
    })
}

pub fn import_eligible_words(
    conn: &mut Connection,
    resolver: &TextResolver,
    content: &[u8],
    expected_count: Option<usize>,
    expected_sha256: Option<&str>,
) -> Result<LexiconImportBatchResult, Error> {
    let hash = Code::Sha2_256.digest(content);
    let digest: String = hash.digest().iter().map(|b| format!("{:02x}", b)).collect();

    if let Some(expected_hash) = expected_sha256 {
        if digest != expected_hash {
            return Err(Error::ValidationError(format!(
                "Expected file SHA2-256 digest to be '{}', but got '{}'.",
                expected_hash, digest
            )));
        }
    }

    let text = std::str::from_utf8(content)
        .map_err(|e| Error::ValidationError(format!("Invalid UTF-8 source file: {:?}", e)))?;

    let lines: Vec<&str> = text.lines().collect();
    let line_count = lines.len();

    let import_id = format!(
        "import-{}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    let source_id =
        "urn:language-graph:lexicon-source:esdb:en-us:rel-2026.02.25:size-60:default-variants";

    // 1. Prepare Metadata and Batch row (outside the main transaction, or in it)
    {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO lexicon_sources (
                source_entity_id, source_kind, label, provider, release_tag, 
                source_path_label, source_configuration_json, license_label, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))",
            params![
                source_id,
                "dictionary-wordlist",
                "ESDB English (US) rel-2026.02.25 Default Wordlist",
                "English Speller Database",
                "rel-2026.02.25",
                "en_US.txt",
                r#"{"dialect": "American English", "esdbSize": 60, "variantPolicy": "default", "expectedSourceEntryCount": 109902, "admissionPolicy": "ascii-letters-with-internal-apostrophe-or-hyphen-v1"}"#,
                "MIT-like",
            ],
        )?;

        tx.execute(
            "INSERT INTO lexicon_import_batches (
                import_id, source_entity_id, source_file_sha256, source_byte_length, 
                source_entry_count, admission_policy, status, started_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))",
            params![
                import_id,
                source_id,
                digest,
                content.len() as i64,
                line_count as i64,
                "ascii-letters-with-internal-apostrophe-or-hyphen-v1",
                "running",
            ],
        )?;
        tx.commit()?;
    }

    if let Some(expected) = expected_count {
        if line_count != expected {
            let _ = conn.execute(
                "UPDATE lexicon_import_batches SET status = 'failed', completed_at = datetime('now') WHERE import_id = ?1",
                params![import_id],
            );
            return Err(Error::ValidationError(format!(
                "Expected exactly {} lines in the wordlist file, but found {}.",
                expected, line_count
            )));
        }
    }

    // 2. Perform bulk import transaction
    let mut eligible_new = 0;
    let mut eligible_reused = 0;
    let mut deferred = 0;
    let mut rejected = 0;

    let import_result = {
        let tx = conn.transaction()?;

        {
            let mut stmt_member = tx.prepare(
                "INSERT OR IGNORE INTO written_form_store_members (store_entity_id, written_form_entity_id, admitted_revision_cid, added_at, status)
                 VALUES (?1, ?2, ?3, datetime('now'), 'active')"
            )?;

            let mut stmt_att = tx.prepare(
                "INSERT INTO written_form_attestations (source_entity_id, written_form_entity_id, first_import_id, latest_import_id, source_surface_form, attestation_status)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'active')
                 ON CONFLICT(source_entity_id, written_form_entity_id) DO UPDATE SET
                     latest_import_id = excluded.latest_import_id,
                     attestation_status = excluded.attestation_status"
            )?;

            let mut stmt_deferred = tx.prepare(
                "INSERT OR IGNORE INTO lexicon_import_deferred_entries (import_id, source_surface_form, normalized_surface_form, reason_code, reason_detail)
                 VALUES (?1, ?2, ?3, ?4, ?5)"
            )?;

            for line in &lines {
                let classification = crate::lexicon_import::esdb::classify_word(line);
                match classification {
                    crate::lexicon_import::esdb::Classification::Eligible => {
                        let normalized: String = line.nfc().collect();
                        let graphemes: Vec<&str> = normalized.graphemes(true).collect();
                        let mut components = Vec::new();
                        let mut supported = true;

                        for (idx, &g) in graphemes.iter().enumerate() {
                            if let Some(cached_info) = resolver.cache.get(g) {
                                components.push(WrittenFormComponent {
                                    position: (idx + 1) as i32,
                                    surface_form: g.to_string(),
                                    symbol_entity_id: cached_info.entity_id.clone(),
                                    symbol_revision_cid: cached_info.revision_cid.clone(),
                                });
                            } else {
                                supported = false;
                                break;
                            }
                        }

                        if !supported {
                            stmt_deferred.execute(params![
                                import_id,
                                *line,
                                normalized,
                                "contains_disallowed_punctuation",
                                "Contains grapheme not supported by the active symbol profile."
                            ])?;
                            deferred += 1;
                            continue;
                        }

                        let (eid, cid_str, is_new) =
                            insert_or_reuse_written_form_within_transaction(
                                &tx,
                                resolver,
                                line,
                                &normalized,
                                &components,
                            )?;

                        stmt_member.execute(params![STORE_ENTITY_ID, eid, cid_str])?;

                        stmt_att.execute(params![source_id, eid, import_id, import_id, *line,])?;

                        if is_new {
                            eligible_new += 1;
                        } else {
                            eligible_reused += 1;
                        }
                    }
                    crate::lexicon_import::esdb::Classification::Deferred {
                        reason_code,
                        reason_detail,
                    } => {
                        let normalized: String = line.nfc().collect();
                        stmt_deferred.execute(params![
                            import_id,
                            *line,
                            normalized,
                            reason_code,
                            reason_detail
                        ])?;

                        if reason_code == "malformed_or_empty" {
                            rejected += 1;
                        } else {
                            deferred += 1;
                        }
                    }
                }
            }
        }

        tx.commit()?;
        Ok::<(), Error>(())
    };

    if let Err(e) = import_result {
        // Update batch status to failed
        let _ = conn.execute(
            "UPDATE lexicon_import_batches SET status = 'failed', completed_at = datetime('now') WHERE import_id = ?1",
            params![import_id],
        );
        return Err(e);
    }

    // 3. Publish snapshot
    let snapshot_res = publish_store_snapshot(conn)?;
    let snapshot_cid = snapshot_res.snapshot_cid;

    // 4. Generate manifest block
    let manifest = LexiconImportManifest {
        schema: "language-graph/lexicon-import-manifest/v1".to_string(),
        source_entity_id: source_id.to_string(),
        source_release_tag: "rel-2026.02.25".to_string(),
        source_file_name: "en_US.txt".to_string(),
        source_file_sha256: digest.clone(),
        source_entry_count: line_count as i64,
        admission_policy: "ascii-letters-with-internal-apostrophe-or-hyphen-v1".to_string(),
        accepted_new_count: eligible_new as i64,
        accepted_reused_count: eligible_reused as i64,
        deferred_count: deferred as i64,
        rejected_or_malformed_count: rejected as i64,
        resulting_store_snapshot_cid: snapshot_cid.clone(),
    };

    let manifest_bytes = to_dag_cbor(&manifest)?;
    let manifest_cid = compute_cid(&manifest_bytes)?;
    let manifest_cid_str = manifest_cid.to_string();

    // 5. Update batch to completed
    {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO immutable_blocks (cid, codec, multihash_algorithm, block_kind, bytes, stored_at)
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
            params![
                manifest_cid_str,
                "dag-cbor",
                "sha2-256",
                "lexicon_import_manifest",
                manifest_bytes
            ],
        )?;

        tx.execute(
            "UPDATE lexicon_import_batches 
             SET status = 'completed', 
                 accepted_new_count = ?1, 
                 accepted_reused_count = ?2, 
                 deferred_count = ?3, 
                 rejected_or_malformed_count = ?4, 
                 published_store_snapshot_cid = ?5, 
                 manifest_cid = ?6, 
                 completed_at = datetime('now') 
             WHERE import_id = ?7",
            params![
                eligible_new as i64,
                eligible_reused as i64,
                deferred as i64,
                rejected as i64,
                snapshot_cid,
                manifest_cid_str,
                import_id
            ],
        )?;
        tx.commit()?;
    }

    Ok(LexiconImportBatchResult {
        source: "ESDB English (US) rel-2026.02.25 Default Wordlist".to_string(),
        entries_read: line_count,
        eligible_new_words: eligible_new,
        eligible_existing_words_to_reuse: eligible_reused,
        deferred_entries: deferred,
        rejected_or_malformed: rejected,
        source_sha256_digest: digest,
        snapshot_cid: Some(snapshot_cid),
        manifest_cid: Some(manifest_cid_str),
    })
}

use rusqlite::OptionalExtension;

pub fn list_lexicon_sources(
    conn: &Connection,
) -> Result<Vec<crate::lexicon_import::provenance::LexiconSource>, Error> {
    let mut stmt = conn.prepare(
        "SELECT source_entity_id, source_kind, label, provider, release_tag, source_path_label, source_configuration_json, license_label, created_at
         FROM lexicon_sources
         ORDER BY created_at DESC"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(crate::lexicon_import::provenance::LexiconSource {
            source_entity_id: row.get(0)?,
            source_kind: row.get(1)?,
            label: row.get(2)?,
            provider: row.get(3)?,
            release_tag: row.get(4)?,
            source_path_label: row.get(5)?,
            source_configuration_json: row.get(6)?,
            license_label: row.get(7)?,
            created_at: row.get(8)?,
        })
    })?;
    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn list_import_batches(
    conn: &Connection,
    limit: i64,
    offset: i64,
) -> Result<Vec<crate::lexicon_import::provenance::LexiconImportBatch>, Error> {
    let mut stmt = conn.prepare(
        "SELECT import_id, source_entity_id, source_file_sha256, source_byte_length, source_entry_count, admission_policy, status, accepted_new_count, accepted_reused_count, deferred_count, rejected_or_malformed_count, published_store_snapshot_cid, manifest_cid, started_at, completed_at
         FROM lexicon_import_batches
         ORDER BY started_at DESC
         LIMIT ?1 OFFSET ?2"
    )?;
    let rows = stmt.query_map([limit, offset], |row| {
        Ok(crate::lexicon_import::provenance::LexiconImportBatch {
            import_id: row.get(0)?,
            source_entity_id: row.get(1)?,
            source_file_sha256: row.get(2)?,
            source_byte_length: row.get(3)?,
            source_entry_count: row.get(4)?,
            admission_policy: row.get(5)?,
            status: row.get(6)?,
            accepted_new_count: row.get(7)?,
            accepted_reused_count: row.get(8)?,
            deferred_count: row.get(9)?,
            rejected_or_malformed_count: row.get(10)?,
            published_store_snapshot_cid: row.get(11)?,
            manifest_cid: row.get(12)?,
            started_at: row.get(13)?,
            completed_at: row.get(14)?,
        })
    })?;
    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn get_import_batch(
    conn: &Connection,
    import_id: &str,
) -> Result<Option<crate::lexicon_import::provenance::LexiconImportBatch>, Error> {
    let batch = conn.query_row(
        "SELECT import_id, source_entity_id, source_file_sha256, source_byte_length, source_entry_count, admission_policy, status, accepted_new_count, accepted_reused_count, deferred_count, rejected_or_malformed_count, published_store_snapshot_cid, manifest_cid, started_at, completed_at
         FROM lexicon_import_batches
         WHERE import_id = ?1",
        [import_id],
        |row| {
            Ok(crate::lexicon_import::provenance::LexiconImportBatch {
                import_id: row.get(0)?,
                source_entity_id: row.get(1)?,
                source_file_sha256: row.get(2)?,
                source_byte_length: row.get(3)?,
                source_entry_count: row.get(4)?,
                admission_policy: row.get(5)?,
                status: row.get(6)?,
                accepted_new_count: row.get(7)?,
                accepted_reused_count: row.get(8)?,
                deferred_count: row.get(9)?,
                rejected_or_malformed_count: row.get(10)?,
                published_store_snapshot_cid: row.get(11)?,
                manifest_cid: row.get(12)?,
                started_at: row.get(13)?,
                completed_at: row.get(14)?,
            })
        }
    ).optional()?;
    Ok(batch)
}

pub fn list_deferred_entries(
    conn: &Connection,
    import_id: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<crate::lexicon_import::provenance::DeferredLexiconEntry>, Error> {
    let mut stmt = conn.prepare(
        "SELECT import_id, source_surface_form, normalized_surface_form, reason_code, reason_detail
         FROM lexicon_import_deferred_entries
         WHERE import_id = ?1
         ORDER BY source_surface_form ASC
         LIMIT ?2 OFFSET ?3",
    )?;
    let rows = stmt.query_map(params![import_id, limit, offset], |row| {
        Ok(crate::lexicon_import::provenance::DeferredLexiconEntry {
            import_id: row.get(0)?,
            source_surface_form: row.get(1)?,
            normalized_surface_form: row.get(2)?,
            reason_code: row.get(3)?,
            reason_detail: row.get(4)?,
        })
    })?;
    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}
