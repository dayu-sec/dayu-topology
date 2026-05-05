use orion_error::{conversion::ToStructError, prelude::*};
use topology_api::ApiReason;
use topology_domain::DomainReason;
use topology_storage::StorageReason;

pub type AppError = StructError<AppReason>;
pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Clone, PartialEq, OrionError)]
pub enum AppReason {
    #[orion_error(identity = "biz.dayu.app.invalid_args")]
    InvalidArgs,
    #[orion_error(identity = "sys.dayu.app.input_load_failed")]
    InputLoadFailed,
    #[orion_error(identity = "logic.dayu.app.materialization_missing")]
    MaterializationMissing,
    #[orion_error(transparent)]
    General(UnifiedReason),
}

impl From<ApiReason> for AppReason {
    fn from(value: ApiReason) -> Self {
        match value {
            ApiReason::General(reason) => AppReason::General(reason),
            _ => AppReason::InputLoadFailed,
        }
    }
}

impl From<StorageReason> for AppReason {
    fn from(value: StorageReason) -> Self {
        match value {
            StorageReason::General(reason) => AppReason::General(reason),
            _ => AppReason::InputLoadFailed,
        }
    }
}

impl From<DomainReason> for AppReason {
    fn from(value: DomainReason) -> Self {
        match value {
            DomainReason::General(reason) => AppReason::General(reason),
            _ => AppReason::InputLoadFailed,
        }
    }
}

pub fn invalid_args() -> AppError {
    AppReason::InvalidArgs
        .to_err()
        .with_detail("usage: topology-app [demo] | [file <path>] | [postgres-mock]")
}

pub fn input_load_failed(detail: impl Into<String>) -> AppError {
    AppReason::InputLoadFailed.to_err().with_detail(detail)
}

pub fn materialization_missing(detail: impl Into<String>) -> AppError {
    AppReason::MaterializationMissing
        .to_err()
        .with_detail(detail)
}
