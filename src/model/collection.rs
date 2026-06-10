use serde::{Deserialize, Serialize};

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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileCollectionRef {
    pub position: i32,
    pub collection_entity_id: String,
    pub snapshot_cid: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TextProfileSnapshot {
    pub schema: String,
    pub profile_entity_id: String,
    pub kind: String,
    pub label: String,
    pub collections: Vec<ProfileCollectionRef>,
}
