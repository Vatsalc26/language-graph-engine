use crate::error::Error;
use cid::Cid;
use multihash_codetable::{Code, MultihashDigest};

pub const DAG_CBOR_CODEC: u64 = 0x71;

pub fn compute_cid(bytes: &[u8]) -> Result<Cid, Error> {
    // Generate SHA2-256 multihash
    let hash = Code::Sha2_256.digest(bytes);

    // Convert to cid's expected Multihash type
    // Depending on version alignment, they might be directly compatible
    // or we might need to convert via bytes or custom mapping.
    // Let's first try direct compatibility since both use multihash 0.19.
    let cid_multihash = cid::multihash::Multihash::from_bytes(&hash.to_bytes())
        .map_err(|e| Error::CidError(format!("Failed to parse multihash: {:?}", e)))?;

    Ok(Cid::new_v1(DAG_CBOR_CODEC, cid_multihash))
}
