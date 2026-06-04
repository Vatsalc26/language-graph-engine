use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Database error: {0}")]
    DbError(#[from] rusqlite::Error),

    #[error("Serialization/Deserialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("DAG-CBOR error: {0}")]
    CborError(String),

    #[error("CID error: {0}")]
    CidError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Integrity error: {0}")]
    IntegrityError(String),

    #[error("Not found error: {0}")]
    NotFoundError(String),
}
