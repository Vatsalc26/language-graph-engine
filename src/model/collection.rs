use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotMember {
    pub position: i32,
    pub entity_id: String,
    pub revision_cid: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AlphabetSnapshot {
    pub schema: String,
    pub collection_entity_id: String,
    pub kind: String,
    pub label: String,
    pub members: Vec<SnapshotMember>,
}
