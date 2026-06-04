use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphemeRevision {
    pub schema: String,
    pub entity_id: String,
    pub kind: String,
    pub surface_form: String,
    pub normalized_form: String,
    pub normalization: String,
    pub unicode_scalars: Vec<String>,
    pub script: String,
    pub case: String,
    pub previous_revision_cid: Option<String>,
}
