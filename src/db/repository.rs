use crate::content::encoding::from_dag_cbor;
use crate::error::Error;
use crate::model::{
    AlphabetSnapshot, GraphemeRevision, ProfileCollectionRef, SnapshotMember, TextProfileSnapshot,
};
use rusqlite::{params, Connection, OptionalExtension};

pub struct Repository<'a> {
    conn: &'a Connection,
}

impl<'a> Repository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    // --- Block Storage ---

    pub fn insert_block(
        &self,
        cid: &str,
        codec: &str,
        multihash_algorithm: &str,
        block_kind: &str,
        bytes: &[u8],
    ) -> Result<(), Error> {
        self.conn.execute(
            "INSERT OR IGNORE INTO immutable_blocks (cid, codec, multihash_algorithm, block_kind, bytes, stored_at)
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
            params![cid, codec, multihash_algorithm, block_kind, bytes],
        )?;
        Ok(())
    }

    pub fn get_block_bytes(&self, cid: &str) -> Result<Option<Vec<u8>>, Error> {
        let bytes: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT bytes FROM immutable_blocks WHERE cid = ?1",
                params![cid],
                |row| row.get(0),
            )
            .optional()?;
        Ok(bytes)
    }

    pub fn block_exists(&self, cid: &str) -> Result<bool, Error> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM immutable_blocks WHERE cid = ?1",
            params![cid],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    // --- Entities ---

    pub fn insert_entity(
        &self,
        entity_id: &str,
        kind: &str,
        canonical_key: &str,
        label: &str,
    ) -> Result<(), Error> {
        // If entity exists, check if canonical_key or other fields conflict
        let existing: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT canonical_key, label FROM entities WHERE entity_id = ?1",
                params![entity_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;

        if let Some((existing_key, existing_label)) = existing {
            if existing_key != canonical_key {
                return Err(Error::IntegrityError(format!(
                    "Conflicting canonical key for entity {}: existing='{}', requested='{}'",
                    entity_id, existing_key, canonical_key
                )));
            }
            if existing_label != label {
                return Err(Error::IntegrityError(format!(
                    "Conflicting label for entity {}: existing='{}', requested='{}'",
                    entity_id, existing_label, label
                )));
            }
        } else {
            self.conn.execute(
                "INSERT INTO entities (entity_id, kind, canonical_key, label, created_at)
                 VALUES (?1, ?2, ?3, ?4, datetime('now'))",
                params![entity_id, kind, canonical_key, label],
            )?;
        }
        Ok(())
    }

    pub fn get_entity_head(&self, entity_id: &str) -> Result<Option<String>, Error> {
        let head: Option<String> = self
            .conn
            .query_row(
                "SELECT revision_cid FROM entity_heads WHERE entity_id = ?1",
                params![entity_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(head)
    }

    pub fn set_entity_head(&self, entity_id: &str, revision_cid: &str) -> Result<(), Error> {
        // First check if there is an existing head
        let existing = self.get_entity_head(entity_id)?;
        if let Some(ref current) = existing {
            if current != revision_cid {
                // In Phase 1, we seed deterministic revisions.
                // Setting it to a different revision CID is not allowed or should raise integrity check if it conflicts.
                // We'll update the head, but log or check if it's seeding conflict.
                // Actually, the prompt says "If the database already contains conflicting canonical seeded data, do not silently overwrite it; report a clear integrity error."
                // So if we are writing a head, but the head has changed, we should throw an integrity error if we are re-seeding.
                return Err(Error::IntegrityError(format!(
                    "Conflicting head revision for entity {}: existing='{}', requested='{}'",
                    entity_id, current, revision_cid
                )));
            }
        } else {
            self.conn.execute(
                "INSERT INTO entity_heads (entity_id, revision_cid, updated_at)
                 VALUES (?1, ?2, datetime('now'))",
                params![entity_id, revision_cid],
            )?;
        }
        Ok(())
    }

    // --- Collections ---

    pub fn insert_collection(
        &self,
        collection_entity_id: &str,
        canonical_key: &str,
        label: &str,
    ) -> Result<(), Error> {
        let existing: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT canonical_key, label FROM collections WHERE collection_entity_id = ?1",
                params![collection_entity_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;

        if let Some((existing_key, existing_label)) = existing {
            if existing_key != canonical_key {
                return Err(Error::IntegrityError(format!(
                    "Conflicting canonical key for collection {}: existing='{}', requested='{}'",
                    collection_entity_id, existing_key, canonical_key
                )));
            }
            if existing_label != label {
                return Err(Error::IntegrityError(format!(
                    "Conflicting label for collection {}: existing='{}', requested='{}'",
                    collection_entity_id, existing_label, label
                )));
            }
        } else {
            self.conn.execute(
                "INSERT INTO collections (collection_entity_id, canonical_key, label, created_at)
                 VALUES (?1, ?2, ?3, datetime('now'))",
                params![collection_entity_id, canonical_key, label],
            )?;
        }
        Ok(())
    }

    pub fn insert_snapshot(
        &self,
        snapshot_cid: &str,
        collection_entity_id: &str,
    ) -> Result<(), Error> {
        let exists: bool = self.conn.query_row(
            "SELECT COUNT(*) FROM collection_snapshots WHERE snapshot_cid = ?1",
            params![snapshot_cid],
            |row| row.get::<_, i64>(0),
        )? > 0;

        if !exists {
            self.conn.execute(
                "INSERT INTO collection_snapshots (snapshot_cid, collection_entity_id, created_at)
                 VALUES (?1, ?2, datetime('now'))",
                params![snapshot_cid, collection_entity_id],
            )?;
        }
        Ok(())
    }

    pub fn insert_snapshot_member(
        &self,
        snapshot_cid: &str,
        position: i32,
        entity_id: &str,
        revision_cid: &str,
    ) -> Result<(), Error> {
        let existing: Option<String> = self.conn.query_row(
            "SELECT revision_cid FROM collection_snapshot_members WHERE snapshot_cid = ?1 AND position = ?2",
            params![snapshot_cid, position],
            |row| row.get(0),
        ).optional()?;

        if let Some(ref r_cid) = existing {
            if r_cid != revision_cid {
                return Err(Error::IntegrityError(format!(
                    "Conflict in snapshot member at position {}: existing revision='{}', requested='{}'",
                    position, r_cid, revision_cid
                )));
            }
        } else {
            self.conn.execute(
                "INSERT INTO collection_snapshot_members (snapshot_cid, position, entity_id, revision_cid)
                 VALUES (?1, ?2, ?3, ?4)",
                params![snapshot_cid, position, entity_id, revision_cid],
            )?;
        }
        Ok(())
    }

    pub fn get_active_snapshot_cid(
        &self,
        collection_entity_id: &str,
    ) -> Result<Option<String>, Error> {
        let cid: Option<String> = self.conn.query_row(
            "SELECT snapshot_cid FROM active_collection_snapshots WHERE collection_entity_id = ?1",
            params![collection_entity_id],
            |row| row.get(0),
        ).optional()?;
        Ok(cid)
    }

    pub fn set_active_snapshot(
        &self,
        collection_entity_id: &str,
        snapshot_cid: &str,
    ) -> Result<(), Error> {
        let existing = self.get_active_snapshot_cid(collection_entity_id)?;
        if let Some(ref current) = existing {
            if current != snapshot_cid {
                // If it conflicts, throw an integrity error during seeding or update it if allowed.
                // In Phase 1 seeding, it should match since the seed is deterministic.
                return Err(Error::IntegrityError(format!(
                    "Conflicting active snapshot for collection {}: existing='{}', requested='{}'",
                    collection_entity_id, current, snapshot_cid
                )));
            }
        } else {
            self.conn.execute(
                "INSERT INTO active_collection_snapshots (collection_entity_id, snapshot_cid, activated_at)
                 VALUES (?1, ?2, datetime('now'))",
                params![collection_entity_id, snapshot_cid],
            )?;
        }
        Ok(())
    }

    pub fn get_snapshot_members(&self, snapshot_cid: &str) -> Result<Vec<SnapshotMember>, Error> {
        let mut stmt = self.conn.prepare(
            "SELECT position, entity_id, revision_cid 
             FROM collection_snapshot_members 
             WHERE snapshot_cid = ?1 
             ORDER BY position ASC",
        )?;
        let rows = stmt.query_map([snapshot_cid], |row| {
            Ok(SnapshotMember {
                position: row.get(0)?,
                entity_id: row.get(1)?,
                revision_cid: row.get(2)?,
            })
        })?;
        let mut members = Vec::new();
        for member in rows {
            members.push(member?);
        }
        Ok(members)
    }

    // --- Complex Queries ---

    pub fn get_grapheme_revision(&self, revision_cid: &str) -> Result<GraphemeRevision, Error> {
        let bytes = self.get_block_bytes(revision_cid)?.ok_or_else(|| {
            Error::NotFoundError(format!("Block not found for CID: {}", revision_cid))
        })?;

        // Assert cryptographic integrity
        let computed_cid = crate::content::cid::compute_cid(&bytes)?;
        if computed_cid.to_string() != revision_cid {
            return Err(Error::IntegrityError(format!(
                "Integrity check failed: requested CID '{}' but block bytes re-hashed to '{}'",
                revision_cid, computed_cid
            )));
        }

        let rev: GraphemeRevision = from_dag_cbor(&bytes)?;
        Ok(rev)
    }

    pub fn get_alphabet_snapshot(&self, snapshot_cid: &str) -> Result<AlphabetSnapshot, Error> {
        let bytes = self.get_block_bytes(snapshot_cid)?.ok_or_else(|| {
            Error::NotFoundError(format!("Block not found for CID: {}", snapshot_cid))
        })?;

        // Assert cryptographic integrity
        let computed_cid = crate::content::cid::compute_cid(&bytes)?;
        if computed_cid.to_string() != snapshot_cid {
            return Err(Error::IntegrityError(format!(
                "Integrity check failed: requested CID '{}' but block bytes re-hashed to '{}'",
                snapshot_cid, computed_cid
            )));
        }

        let snap: AlphabetSnapshot = from_dag_cbor(&bytes)?;
        Ok(snap)
    }

    // --- Profiles ---

    pub fn insert_profile(
        &self,
        profile_entity_id: &str,
        canonical_key: &str,
        label: &str,
    ) -> Result<(), Error> {
        let existing: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT canonical_key, label FROM text_profiles WHERE profile_entity_id = ?1",
                params![profile_entity_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;

        if let Some((existing_key, existing_label)) = existing {
            if existing_key != canonical_key {
                return Err(Error::IntegrityError(format!(
                    "Conflicting canonical key for profile {}: existing='{}', requested='{}'",
                    profile_entity_id, existing_key, canonical_key
                )));
            }
            if existing_label != label {
                return Err(Error::IntegrityError(format!(
                    "Conflicting label for profile {}: existing='{}', requested='{}'",
                    profile_entity_id, existing_label, label
                )));
            }
        } else {
            self.conn.execute(
                "INSERT INTO text_profiles (profile_entity_id, canonical_key, label, created_at)
                 VALUES (?1, ?2, ?3, datetime('now'))",
                params![profile_entity_id, canonical_key, label],
            )?;
        }
        Ok(())
    }

    pub fn insert_profile_snapshot(
        &self,
        snapshot_cid: &str,
        profile_entity_id: &str,
    ) -> Result<(), Error> {
        let exists: bool = self.conn.query_row(
            "SELECT COUNT(*) FROM text_profile_snapshots WHERE snapshot_cid = ?1",
            params![snapshot_cid],
            |row| row.get::<_, i64>(0),
        )? > 0;

        if !exists {
            self.conn.execute(
                "INSERT INTO text_profile_snapshots (snapshot_cid, profile_entity_id, created_at)
                 VALUES (?1, ?2, datetime('now'))",
                params![snapshot_cid, profile_entity_id],
            )?;
        }
        Ok(())
    }

    pub fn insert_profile_snapshot_collection(
        &self,
        profile_snapshot_cid: &str,
        position: i32,
        collection_entity_id: &str,
        collection_snapshot_cid: &str,
    ) -> Result<(), Error> {
        let existing: Option<String> = self.conn.query_row(
            "SELECT collection_snapshot_cid FROM text_profile_snapshot_collections WHERE profile_snapshot_cid = ?1 AND position = ?2",
            params![profile_snapshot_cid, position],
            |row| row.get(0),
        ).optional()?;

        if let Some(ref c_cid) = existing {
            if c_cid != collection_snapshot_cid {
                return Err(Error::IntegrityError(format!(
                    "Conflict in profile snapshot collection at position {}: existing collection snapshot='{}', requested='{}'",
                    position, c_cid, collection_snapshot_cid
                )));
            }
        } else {
            self.conn.execute(
                "INSERT INTO text_profile_snapshot_collections (profile_snapshot_cid, position, collection_entity_id, collection_snapshot_cid)
                 VALUES (?1, ?2, ?3, ?4)",
                params![profile_snapshot_cid, position, collection_entity_id, collection_snapshot_cid],
            )?;
        }
        Ok(())
    }

    pub fn set_active_profile_snapshot(
        &self,
        profile_entity_id: &str,
        snapshot_cid: &str,
    ) -> Result<(), Error> {
        let existing = self.get_active_profile_snapshot_cid(profile_entity_id)?;
        if let Some(ref current) = existing {
            if current != snapshot_cid {
                return Err(Error::IntegrityError(format!(
                    "Conflicting active snapshot for profile {}: existing='{}', requested='{}'",
                    profile_entity_id, current, snapshot_cid
                )));
            }
        } else {
            self.conn.execute(
                "INSERT INTO active_text_profile_snapshots (profile_entity_id, snapshot_cid, activated_at)
                 VALUES (?1, ?2, datetime('now'))",
                params![profile_entity_id, snapshot_cid],
            )?;
        }
        Ok(())
    }

    pub fn get_active_profile_snapshot_cid(
        &self,
        profile_entity_id: &str,
    ) -> Result<Option<String>, Error> {
        let cid: Option<String> = self.conn.query_row(
            "SELECT snapshot_cid FROM active_text_profile_snapshots WHERE profile_entity_id = ?1",
            params![profile_entity_id],
            |row| row.get(0),
        ).optional()?;
        Ok(cid)
    }

    pub fn get_profile_collections(
        &self,
        profile_snapshot_cid: &str,
    ) -> Result<Vec<ProfileCollectionRef>, Error> {
        let mut stmt = self.conn.prepare(
            "SELECT position, collection_entity_id, collection_snapshot_cid
             FROM text_profile_snapshot_collections
             WHERE profile_snapshot_cid = ?1
             ORDER BY position ASC",
        )?;
        let rows = stmt.query_map([profile_snapshot_cid], |row| {
            Ok(ProfileCollectionRef {
                position: row.get(0)?,
                collection_entity_id: row.get(1)?,
                snapshot_cid: row.get(2)?,
            })
        })?;
        let mut collections = Vec::new();
        for collection in rows {
            collections.push(collection?);
        }
        Ok(collections)
    }

    pub fn get_profile_snapshot(&self, snapshot_cid: &str) -> Result<TextProfileSnapshot, Error> {
        let bytes = self.get_block_bytes(snapshot_cid)?.ok_or_else(|| {
            Error::NotFoundError(format!("Block not found for CID: {}", snapshot_cid))
        })?;

        // Assert cryptographic integrity
        let computed_cid = crate::content::cid::compute_cid(&bytes)?;
        if computed_cid.to_string() != snapshot_cid {
            return Err(Error::IntegrityError(format!(
                "Integrity check failed: requested CID '{}' but block bytes re-hashed to '{}'",
                snapshot_cid, computed_cid
            )));
        }

        let snap: TextProfileSnapshot = from_dag_cbor(&bytes)?;
        Ok(snap)
    }
}
