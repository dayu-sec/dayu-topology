use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    Confidence, EnvironmentId, ObservedAt, ResponsibilityRole, ServiceBoundary, ServiceType,
    SourceKind, SubjectType, TenantId, ValidityWindow, WorkloadKind,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IngestEnvelope {
    pub ingest_id: String,
    pub source_kind: SourceKind,
    pub source_name: String,
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub observed_at: Option<ObservedAt>,
    pub received_at: DateTime<Utc>,
    pub payload_ref: Option<String>,
    pub payload_inline: Option<Value>,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BusinessCatalogCandidate {
    pub tenant_id: TenantId,
    pub source_kind: SourceKind,
    pub external_ref: Option<String>,
    pub business_name: String,
    pub system_name: Option<String>,
    pub subsystem_name: Option<String>,
    pub service_name: Option<String>,
    pub service_type: Option<ServiceType>,
    pub boundary: Option<ServiceBoundary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostCandidate {
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub source_kind: SourceKind,
    pub external_ref: Option<String>,
    pub host_name: String,
    pub machine_id: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubjectCandidate {
    pub tenant_id: TenantId,
    pub source_kind: SourceKind,
    pub subject_type: SubjectType,
    pub external_ref: Option<String>,
    pub display_name: String,
    pub email: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkloadCandidate {
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub source_kind: SourceKind,
    pub cluster_name: String,
    pub namespace_name: String,
    pub workload_kind: WorkloadKind,
    pub workload_name: String,
    pub service_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResponsibilityAssignmentCandidate {
    pub tenant_id: TenantId,
    pub source_kind: SourceKind,
    pub subject_external_ref: Option<String>,
    pub subject_email: Option<String>,
    pub target_external_ref: Option<String>,
    pub role: ResponsibilityRole,
    pub validity: ValidityWindow,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolutionCandidate {
    pub source_kind: SourceKind,
    pub rule_hints: Vec<String>,
    pub matched_identifiers: BTreeMap<String, String>,
    pub confidence: Confidence,
}
