use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LexiconImportBatchResult {
    pub source: String,
    pub entries_read: usize,
    pub eligible_new_words: usize,
    pub eligible_existing_words_to_reuse: usize,
    pub deferred_entries: usize,
    pub rejected_or_malformed: usize,
    pub source_sha256_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_cid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest_cid: Option<String>,
}
