use thiserror::Error;
use topology_domain::{BusinessDomain, HostInventory, ResponsibilityAssignment, ServiceEntity};

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("storage backend is not configured")]
    NotConfigured,
}

pub trait TopologyCatalogStore {
    fn upsert_business(&self, business: &BusinessDomain) -> Result<(), StorageError>;
    fn upsert_host(&self, host: &HostInventory) -> Result<(), StorageError>;
    fn upsert_service(&self, service: &ServiceEntity) -> Result<(), StorageError>;
    fn upsert_assignment(&self, assignment: &ResponsibilityAssignment) -> Result<(), StorageError>;
}
