use crate::content::cid::compute_cid;
use crate::content::encoding::{from_dag_cbor, to_dag_cbor};
use crate::error::Error;
use crate::model::{WrittenFormStoreSnapshot, WrittenFormStoreSnapshotMember};
use crate::written_forms::repository::STORE_ENTITY_ID;
use rusqlite::{params, Connection, OptionalExtension};

#[derive(serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PublishResult {
    pub snapshot_cid: String,
    pub store_entity_id: String,
    pub member_count: usize,
    pub status: String,
}

pub fn publish_store_snapshot(conn: &mut Connection) -> Result<PublishResult, Error> {
    let members = {
        let mut stmt = conn.prepare(
            "SELECT wfsm.written_form_entity_id, wfsm.admitted_revision_cid
             FROM written_form_store_members wfsm
             JOIN written_forms wf ON wfsm.written_form_entity_id = wf.entity_id
             WHERE wfsm.store_entity_id = ?1 AND wfsm.status = 'active'
             ORDER BY wf.normalized_surface_form COLLATE BINARY ASC, wfsm.written_form_entity_id COLLATE BINARY ASC",
        )?;

        let rows = stmt.query_map([STORE_ENTITY_ID], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut members = Vec::new();
        for (idx, row) in rows.enumerate() {
            let (entity_id, revision_cid) = row?;
            members.push(WrittenFormStoreSnapshotMember {
                position: (idx + 1) as i32,
                written_form_entity_id: entity_id,
                written_form_revision_cid: revision_cid,
            });
        }
        members
    };

    let member_count = members.len();

    let snapshot = WrittenFormStoreSnapshot {
        schema: "language-graph/written-form-store-snapshot/v1".to_string(),
        store_entity_id: STORE_ENTITY_ID.to_string(),
        kind: "written-form-store-snapshot".to_string(),
        label: "English Natural-Language Written Forms".to_string(),
        members,
    };

    let bytes = to_dag_cbor(&snapshot)?;
    let cid = compute_cid(&bytes)?;
    let cid_str = cid.to_string();

    let block_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM immutable_blocks WHERE cid = ?1",
        [&cid_str],
        |row| row.get::<_, i64>(0),
    )? > 0;

    let tx = conn.transaction()?;

    if block_exists {
        tx.execute(
            "INSERT OR REPLACE INTO active_written_form_store_snapshots (store_entity_id, snapshot_cid, activated_at)
             VALUES (?1, ?2, datetime('now'))",
            params![STORE_ENTITY_ID, cid_str],
        )?;
        tx.commit()?;
        return Ok(PublishResult {
            snapshot_cid: cid_str,
            store_entity_id: STORE_ENTITY_ID.to_string(),
            member_count,
            status: "No Changes".to_string(),
        });
    }

    tx.execute(
        "INSERT INTO immutable_blocks (cid, codec, multihash_algorithm, block_kind, bytes, stored_at)
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
        params![cid_str, "dag-cbor", "sha2-256", "written_form_store_snapshot", bytes],
    )?;

    tx.execute(
        "INSERT INTO written_form_store_snapshots (snapshot_cid, store_entity_id, created_at)
         VALUES (?1, ?2, datetime('now'))",
        params![cid_str, STORE_ENTITY_ID],
    )?;

    for member in &snapshot.members {
        tx.execute(
            "INSERT INTO written_form_store_snapshot_members (snapshot_cid, position, written_form_entity_id, written_form_revision_cid)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                cid_str,
                member.position,
                member.written_form_entity_id,
                member.written_form_revision_cid
            ],
        )?;
    }

    tx.execute(
        "INSERT OR REPLACE INTO active_written_form_store_snapshots (store_entity_id, snapshot_cid, activated_at)
         VALUES (?1, ?2, datetime('now'))",
        params![STORE_ENTITY_ID, cid_str],
    )?;

    tx.commit()?;

    Ok(PublishResult {
        snapshot_cid: cid_str,
        store_entity_id: STORE_ENTITY_ID.to_string(),
        member_count,
        status: "Published".to_string(),
    })
}

pub fn get_active_store_snapshot(
    conn: &rusqlite::Connection,
) -> Result<Option<WrittenFormStoreSnapshot>, Error> {
    let snapshot_cid: Option<String> = conn
        .query_row(
            "SELECT snapshot_cid FROM active_written_form_store_snapshots WHERE store_entity_id = ?1",
            [STORE_ENTITY_ID],
            |row| row.get(0),
        )
        .optional()?;

    if let Some(cid_str) = snapshot_cid {
        let bytes = conn.query_row(
            "SELECT bytes FROM immutable_blocks WHERE cid = ?1",
            [&cid_str],
            |row| row.get::<_, Vec<u8>>(0),
        )?;

        let snapshot: WrittenFormStoreSnapshot = from_dag_cbor(&bytes)?;
        Ok(Some(snapshot))
    } else {
        Ok(None)
    }
}
