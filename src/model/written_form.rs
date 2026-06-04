use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WrittenFormComponent {
    pub position: i32,
    pub surface_form: String,
    pub symbol_entity_id: String,
    pub symbol_revision_cid: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WrittenFormRevision {
    pub schema: String,
    pub entity_id: String,
    pub kind: String,
    pub form_class: String,
    pub surface_form: String,
    pub normalized_form: String,
    pub normalization: String,
    pub composition_profile_snapshot_cid: String,
    pub components: Vec<WrittenFormComponent>,
    pub previous_revision_cid: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WrittenFormStoreSnapshotMember {
    pub position: i32,
    pub written_form_entity_id: String,
    pub written_form_revision_cid: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WrittenFormStoreSnapshot {
    pub schema: String,
    pub store_entity_id: String,
    pub kind: String,
    pub label: String,
    pub members: Vec<WrittenFormStoreSnapshotMember>,
}
