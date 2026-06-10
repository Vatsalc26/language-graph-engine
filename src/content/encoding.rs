use crate::error::Error;

pub fn to_dag_cbor<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    serde_ipld_dagcbor::to_vec(value)
        .map_err(|e| Error::CborError(format!("DAG-CBOR serialization failed: {:?}", e)))
}

pub fn from_dag_cbor<'a, T: serde::Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, Error> {
    serde_ipld_dagcbor::from_slice(bytes)
        .map_err(|e| Error::CborError(format!("DAG-CBOR deserialization failed: {:?}", e)))
}
