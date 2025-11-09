use thiserror::Error;

#[derive(Error, Debug)]
pub enum MPTError {
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Cache error: {0}")]
    CacheError(String),
    
    #[error("Node not found")]
    NodeNotFound,
    
    #[error("Invalid key: {0}")]
    InvalidKey(String),
    
    #[error("Invalid data: {0}")]
    InvalidData(String),
    
    #[error("Lock error: {0}")]
    LockError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}