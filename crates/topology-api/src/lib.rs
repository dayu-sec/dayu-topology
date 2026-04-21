use serde::{Deserialize, Serialize};
use topology_domain::{BusinessDomain, HostInventory, ServiceEntity};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogSummary {
    pub businesses: usize,
    pub hosts: usize,
    pub services: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BusinessView {
    pub business: BusinessDomain,
    pub services: Vec<ServiceEntity>,
    pub hosts: Vec<HostInventory>,
}
