use crate::error::Error;
use crate::model::WrittenFormComponent;
use crate::resolver::text::TextResolver;
use crate::written_forms::policy::is_eligible;
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PreviewResult {
    pub original_input: String,
    pub normalized_form: String,
    pub is_eligible: bool,
    pub validation_message: Option<String>,
    pub expected_entity_id: Option<String>,
    pub active_profile_snapshot_cid: Option<String>,
    pub components: Option<Vec<WrittenFormComponent>>,
    pub is_already_stored: bool,
}

pub fn derive_entity_id(normalized: &str) -> String {
    let bytes = normalized.as_bytes();
    let hex_str: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    format!("urn:language-graph:written-form:nfc:utf8:{}", hex_str)
}

pub fn preview_written_form(
    resolver: &TextResolver,
    conn: &rusqlite::Connection,
    surface_form: &str,
) -> Result<PreviewResult, Error> {
    let normalized: String = surface_form.nfc().collect();
    let eligible = is_eligible(&normalized);

    if !eligible {
        return Ok(PreviewResult {
            original_input: surface_form.to_string(),
            normalized_form: normalized,
            is_eligible: false,
            validation_message: Some("Only ASCII letters with internal apostrophes or hyphens are accepted in this store.".to_string()),
            expected_entity_id: None,
            active_profile_snapshot_cid: None,
            components: None,
            is_already_stored: false,
        });
    }

    // Determine deterministic entity ID
    let entity_id = derive_entity_id(&normalized);

    // Segment into graphemes
    let graphemes: Vec<&str> = normalized.graphemes(true).collect();
    let mut components = Vec::new();

    for (idx, &g) in graphemes.iter().enumerate() {
        if let Some(cached_info) = resolver.cache.get(g) {
            components.push(WrittenFormComponent {
                position: (idx + 1) as i32,
                surface_form: g.to_string(),
                symbol_entity_id: cached_info.entity_id.clone(),
                symbol_revision_cid: cached_info.revision_cid.clone(),
            });
        } else {
            // Fallback for symbols not in active profile cache
            return Ok(PreviewResult {
                original_input: surface_form.to_string(),
                normalized_form: normalized.clone(),
                is_eligible: false,
                validation_message: Some(format!(
                    "Grapheme '{}' is not supported by the active symbol profile.",
                    g
                )),
                expected_entity_id: None,
                active_profile_snapshot_cid: None,
                components: None,
                is_already_stored: false,
            });
        }
    }

    // Check if already stored in database
    let is_already_stored: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM written_forms WHERE normalized_surface_form = ?1 COLLATE BINARY",
            [&normalized],
            |row| row.get::<_, i64>(0),
        )
        .map(|count| count > 0)
        .unwrap_or(false);

    Ok(PreviewResult {
        original_input: surface_form.to_string(),
        normalized_form: normalized,
        is_eligible: true,
        validation_message: None,
        expected_entity_id: Some(entity_id),
        active_profile_snapshot_cid: Some(resolver.active_snapshot_cid.clone()),
        components: Some(components),
        is_already_stored,
    })
}
