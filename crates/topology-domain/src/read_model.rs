use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    BusinessDomain, HostInventory, HostRuntimeState, ResponsibilityAssignment, ServiceEntity,
    ServiceInstance, Subject,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogSummary {
    pub businesses: usize,
    pub systems: usize,
    pub services: usize,
    pub hosts: usize,
    pub subjects: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BusinessOverviewView {
    pub business: BusinessDomain,
    pub services: Vec<ServiceEntity>,
    pub hosts: Vec<HostInventory>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HostTopologyView {
    pub host: HostInventory,
    pub latest_runtime: Option<HostRuntimeState>,
    pub services: Vec<ServiceEntity>,
    pub assignments: Vec<ResponsibilityAssignment>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceTopologyView {
    pub service: ServiceEntity,
    pub instances: Vec<ServiceInstance>,
    pub hosts: Vec<HostInventory>,
    pub assignments: Vec<ResponsibilityAssignment>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EffectiveResponsibilityView {
    pub subject: Subject,
    pub assignment: ResponsibilityAssignment,
    pub generated_at: DateTime<Utc>,
}
