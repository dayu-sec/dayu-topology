pub mod migrations;
pub mod postgres;
pub mod repositories;

pub use migrations::*;
pub use repositories::*;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("storage backend is not configured")]
    NotConfigured,
    #[error("record was not found")]
    NotFound,
    #[error("storage operation failed: {0}")]
    OperationFailed(String),
}

pub type StorageResult<T> = Result<T, StorageError>;
