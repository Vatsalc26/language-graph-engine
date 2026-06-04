use std::collections::{HashMap, HashSet};
use unicode_segmentation::UnicodeSegmentation;
use crate::error::Error;
use crate::db::repository::Repository;
use crate::seed::lowercase_latin::COLLECTION_ENTITY_ID;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphemeCachedInfo {
    pub entity_id: String,
    pub revision_cid: String,
    pub surface_form: String,
}

#[derive(Clone, Debug)]
pub struct TextResolver {
    pub active_snapshot_cid: String,
    pub cache: HashMap<String, GraphemeCachedInfo>,
}

#[derive(serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionStep {
    pub position: usize,
    pub input_grapheme: String,
    pub entity_id: String,
    pub revision_cid: String,
    pub surface_form: String,
    pub status: String, // "Resolved" or "Reused"
}

#[derive(serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionResult {
    pub input: String,
    pub output: String,
    pub collection_snapshot_cid: String,
    pub trace: Vec<ResolutionStep>,
}

impl TextResolver {
    pub fn load(conn: &rusqlite::Connection) -> Result<Self, Error> {
        let repo = Repository::new(conn);
        
        // Find active snapshot CID
        let active_cid = repo.get_active_snapshot_cid(COLLECTION_ENTITY_ID)?
            .ok_or_else(|| Error::NotFoundError("No active snapshot found for lowercase latin alphabet".to_string()))?;

        // Load snapshot members
        let members = repo.get_snapshot_members(&active_cid)?;
        
        let mut cache = HashMap::new();
        for member in members {
            // Load the grapheme revision block to get the surface form
            let rev = repo.get_grapheme_revision(&member.revision_cid)?;
            cache.insert(
                rev.surface_form.clone(),
                GraphemeCachedInfo {
                    entity_id: member.entity_id,
                    revision_cid: member.revision_cid,
                    surface_form: rev.surface_form,
                },
            );
        }

        Ok(Self {
            active_snapshot_cid: active_cid,
            cache,
        })
    }

    pub fn resolve(&self, input: &str) -> Result<ResolutionResult, Error> {
        if input.is_empty() {
            return Err(Error::ValidationError("Input text cannot be empty".to_string()));
        }

        // Segment into graphemes
        let graphemes: Vec<&str> = input.graphemes(true).collect();
        let mut trace = Vec::new();
        let mut seen_graphemes = HashSet::new();
        let mut output = String::new();

        for (idx, &g) in graphemes.iter().enumerate() {
            // Validate grapheme is in the active collection cache
            let cached_info = self.cache.get(g).ok_or_else(|| {
                Error::ValidationError(format!(
                    "Unsupported character or grapheme: '{}'. Phase 1 only supports lowercase Latin letters a-z.",
                    g
                ))
            })?;

            let is_new = seen_graphemes.insert(g.to_string());
            let status = if is_new {
                "Resolved".to_string()
            } else {
                "Reused".to_string()
            };

            trace.push(ResolutionStep {
                position: idx + 1,
                input_grapheme: g.to_string(),
                entity_id: cached_info.entity_id.clone(),
                revision_cid: cached_info.revision_cid.clone(),
                surface_form: cached_info.surface_form.clone(),
                status,
            });

            output.push_str(&cached_info.surface_form);
        }

        Ok(ResolutionResult {
            input: input.to_string(),
            output,
            collection_snapshot_cid: self.active_snapshot_cid.clone(),
            trace,
        })
    }
}
