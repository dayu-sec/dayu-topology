pub mod memory;
pub mod migrations;
pub mod postgres;
pub mod repositories;

pub use memory::*;
pub use migrations::*;
pub use postgres::*;
pub use repositories::*;

use orion_error::{conversion::ToStructError, prelude::*};

pub type StorageError = StructError<StorageReason>;
pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Debug, Clone, PartialEq, OrionError)]
pub enum StorageReason {
    #[orion_error(identity = "conf.dayu.storage.not_configured")]
    NotConfigured,
    #[orion_error(identity = "biz.dayu.storage.not_found")]
    NotFound,
    #[orion_error(identity = "sys.dayu.storage.operation_failed")]
    OperationFailed,
    #[orion_error(transparent)]
    General(UnifiedReason),
}

pub fn not_configured() -> StorageError {
    StorageReason::NotConfigured.to_err()
}

pub fn not_found() -> StorageError {
    StorageReason::NotFound.to_err()
}

pub fn operation_failed(detail: impl Into<String>) -> StorageError {
    StorageReason::OperationFailed.to_err().with_detail(detail)
}
