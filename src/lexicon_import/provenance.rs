use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LexiconSource {
    pub source_entity_id: String,
    pub source_kind: String,
    pub label: String,
    pub provider: String,
    pub release_tag: String,
    pub source_path_label: String,
    pub source_configuration_json: String,
    pub license_label: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LexiconImportBatch {
    pub import_id: String,
    pub source_entity_id: String,
    pub source_file_sha256: String,
    pub source_byte_length: i64,
    pub source_entry_count: i64,
    pub admission_policy: String,
    pub status: String,
    pub accepted_new_count: i64,
    pub accepted_reused_count: i64,
    pub deferred_count: i64,
    pub rejected_or_malformed_count: i64,
    pub published_store_snapshot_cid: Option<String>,
    pub manifest_cid: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeferredLexiconEntry {
    pub import_id: String,
    pub source_surface_form: String,
    pub normalized_surface_form: String,
    pub reason_code: String,
    pub reason_detail: String,
}
