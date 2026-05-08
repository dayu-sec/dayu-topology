use orion_error::{conversion::ToStructError, prelude::*, runtime::OperationContext};
use topology_storage::StorageReason;

pub type SyncError = StructError<SyncReason>;
pub type SyncResult<T> = Result<T, SyncError>;

#[derive(Debug, Clone, PartialEq, OrionError)]
pub enum SyncReason {
    #[orion_error(identity = "sys.dayu.sync.fetch_failed")]
    FetchFailed,
    #[orion_error(identity = "biz.dayu.sync.cursor_conflict")]
    CursorConflict,
    #[orion_error(identity = "sys.dayu.sync.rate_limited")]
    RateLimited,
    #[orion_error(transparent)]
    General(UnifiedReason),
}

impl From<StorageReason> for SyncReason {
    fn from(value: StorageReason) -> Self {
        match value {
            StorageReason::General(reason) => SyncReason::General(reason),
            _ => SyncReason::FetchFailed,
        }
    }
}

pub fn fetch_failed(detail: impl Into<String>) -> SyncError {
    SyncReason::FetchFailed
        .to_err()
        .with_detail(detail)
        .with_context(
            OperationContext::doing("fetch external topology source")
                .with_meta("component.name", "topology-sync"),
        )
}

pub fn cursor_conflict(detail: impl Into<String>) -> SyncError {
    SyncReason::CursorConflict
        .to_err()
        .with_detail(detail)
        .with_context(
            OperationContext::doing("advance sync cursor")
                .with_meta("component.name", "topology-sync"),
        )
}

pub fn rate_limited(detail: impl Into<String>) -> SyncError {
    SyncReason::RateLimited
        .to_err()
        .with_detail(detail)
        .with_context(
            OperationContext::doing("fetch external topology source")
                .with_meta("component.name", "topology-sync"),
        )
}
