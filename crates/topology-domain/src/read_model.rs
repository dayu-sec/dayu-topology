use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    BusinessDomain, HostInventory, HostNetAssoc, HostRuntimeState, NetworkSegment,
    ProcessRuntimeState, ResponsibilityAssignment, ServiceEntity, ServiceInstance, Subject,
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
    pub processes: Vec<ProcessRuntimeState>,
    pub process_groups: Vec<HostProcessGroupView>,
    pub network_segments: Vec<NetworkSegment>,
    pub network_assocs: Vec<HostNetAssoc>,
    pub services: Vec<ServiceEntity>,
    pub assignments: Vec<ResponsibilityAssignment>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostProcessGroupView {
    pub executable: String,
    pub display_name: String,
    pub process_count: usize,
    pub total_memory_rss_kib: i64,
    pub dominant_state: Option<String>,
    pub state_summary: Vec<ProcessStateCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessStateCount {
    pub state: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkTopologyView {
    pub segment: NetworkSegment,
    pub hosts: Vec<HostInventory>,
    pub host_assocs: Vec<HostNetAssoc>,
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
