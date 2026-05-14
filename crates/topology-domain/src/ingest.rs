use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::{
    Confidence, DomainError, DomainReason, DomainResult, EnvironmentId, ObservedAt,
    ResponsibilityRole, ServiceBoundary, ServiceType, SourceKind, SubjectType, TenantId,
    ValidityWindow, WorkloadKind,
};
use orion_error::conversion::ToStructError;

const DAYU_INPUT_SCHEMA_FAMILIES: &[&str] = &[
    "edge",
    "cmdb",
    "iam",
    "k8s",
    "telemetry",
    "sw",
    "artifact",
    "vuln",
    "bug",
    "security",
    "oncall",
    "manual",
    "correction",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DayuInputEnvelope {
    pub schema: String,
    pub source: DayuInputSource,
    pub collect: DayuInputCollect,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DayuInputSource {
    pub system: String,
    pub producer: String,
    #[serde(alias = "tenant_ref")]
    pub tenant: String,
    #[serde(alias = "env_ref")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DayuInputCollect {
    pub mode: DayuInputMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snap_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collected_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_string_like",
        skip_serializing_if = "Option::is_none"
    )]
    pub res_ver: Option<String>,
}

impl DayuInputCollect {
    pub fn collected_or_observed_at(&self) -> Option<DateTime<Utc>> {
        self.collected_at.or(self.observed_at)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DayuInputMode {
    Snapshot,
    Incremental,
    Window,
    Correction,
}

impl DayuInputMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Snapshot => "snapshot",
            Self::Incremental => "incremental",
            Self::Window => "window",
            Self::Correction => "correction",
        }
    }
}

impl DayuInputEnvelope {
    pub fn schema_family(&self) -> Option<&str> {
        let mut parts = self.schema.split('.');
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some("dayu"), Some("in"), Some(family), Some(version))
                if version.starts_with('v') && parts.next().is_none() =>
            {
                Some(family)
            }
            _ => None,
        }
    }

    pub fn validate(&self) -> DomainResult<()> {
        require_non_empty(&self.schema, "schema")?;
        require_non_empty(&self.source.system, "source.system")?;
        require_non_empty(&self.source.producer, "source.producer")?;
        require_non_empty(&self.source.tenant, "source.tenant")?;
        if let Some(env) = self.source.env.as_ref() {
            require_non_empty(env, "source.env")?;
        }

        let family = self
            .schema_family()
            .ok_or_else(|| invalid_schema("schema must match dayu.in.<family>.v<major>"))?;
        if !DAYU_INPUT_SCHEMA_FAMILIES.contains(&family) {
            return Err(DomainReason::SchemaUnsupported
                .to_err()
                .with_detail(format!("schema family `{family}` is not supported")));
        }

        if !self.payload.is_object() {
            return Err(DomainReason::PayloadInvalid
                .to_err()
                .with_detail("payload must be a JSON object"));
        }

        match self.collect.mode {
            DayuInputMode::Snapshot => {
                require_option_non_empty(self.collect.snap_id.as_deref(), "collect.snap_id")?;
                if self.collect.observed_at.is_none() {
                    return Err(missing_field(
                        "collect.observed_at",
                        "collect.observed_at is required for snapshot mode",
                    ));
                }
            }
            DayuInputMode::Window => {
                if self.collect.observed_at.is_none() {
                    return Err(missing_field(
                        "collect.observed_at",
                        "collect.observed_at is required for window mode",
                    ));
                }
                let window = self
                    .payload
                    .get("window")
                    .and_then(Value::as_object)
                    .ok_or_else(|| {
                        missing_field(
                            "payload.window",
                            "payload.window is required for window mode",
                        )
                    })?;
                require_json_string(window.get("start"), "payload.window.start")?;
                require_json_string(window.get("end"), "payload.window.end")?;
            }
            DayuInputMode::Correction => {
                require_json_string(self.payload.get("correction_id"), "payload.correction_id")?;
            }
            DayuInputMode::Incremental => {}
        }

        Ok(())
    }

    pub fn source_kind(&self) -> SourceKind {
        match self.schema_family() {
            Some("edge") | Some("sw") | Some("artifact") => SourceKind::EdgeDiscovery,
            Some("telemetry") | Some("bug") | Some("security") => SourceKind::TelemetrySummary,
            Some("manual") | Some("correction") => SourceKind::Manual,
            Some("cmdb") | Some("iam") | Some("k8s") | Some("vuln") | Some("oncall") => {
                SourceKind::ExternalSync
            }
            _ => SourceKind::ExternalSync,
        }
    }

    pub fn ingest_mode(&self) -> IngestMode {
        match self.collect.mode {
            DayuInputMode::Snapshot => IngestMode::Snapshot,
            DayuInputMode::Incremental => IngestMode::Delta,
            DayuInputMode::Window | DayuInputMode::Correction => IngestMode::BatchUpsert,
        }
    }

    pub fn idempotency_key(&self) -> String {
        match self.collect.mode {
            DayuInputMode::Snapshot => format!(
                "{}:{}:{}:{}:{}:{}",
                self.schema,
                self.source.system,
                self.source.producer,
                self.source.tenant,
                self.source.env.as_deref().unwrap_or(""),
                self.collect.snap_id.as_deref().unwrap_or("")
            ),
            DayuInputMode::Window => {
                let window = self.payload.get("window").and_then(Value::as_object);
                let start = window
                    .and_then(|window| window.get("start"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let end = window
                    .and_then(|window| window.get("end"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                format!(
                    "{}:{}:{}:{}:{}:{}:{}:{}",
                    self.schema,
                    self.source.system,
                    self.source.producer,
                    self.source.tenant,
                    self.source.env.as_deref().unwrap_or(""),
                    self.collect.snap_id.as_deref().unwrap_or(""),
                    start,
                    end
                )
            }
            DayuInputMode::Incremental => format!(
                "{}:{}:{}:{}:{}:{}",
                self.schema,
                self.source.system,
                self.source.producer,
                self.source.tenant,
                self.source.env.as_deref().unwrap_or(""),
                self.collect.cursor.as_deref().unwrap_or("")
            ),
            DayuInputMode::Correction => format!(
                "{}:{}:{}",
                self.schema,
                self.source.system,
                self.payload
                    .get("correction_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            ),
        }
    }

    pub fn into_ingest_envelope(
        self,
        tenant_id: TenantId,
        environment_id: Option<EnvironmentId>,
        received_at: DateTime<Utc>,
    ) -> IngestEnvelope {
        let observed_at = self.collect.observed_at.map(ObservedAt);
        let received_at = self
            .collect
            .collected_or_observed_at()
            .unwrap_or(received_at);
        let ingest_id = self.idempotency_key();
        let source_kind = self.source_kind();
        let ingest_mode = self.ingest_mode();
        let mut metadata = BTreeMap::new();
        metadata.insert("schema".to_string(), self.schema.clone());
        metadata.insert("source.system".to_string(), self.source.system.clone());
        metadata.insert("source.producer".to_string(), self.source.producer.clone());
        metadata.insert("source.tenant".to_string(), self.source.tenant.clone());
        if let Some(env) = self.source.env.as_ref() {
            metadata.insert("source.env".to_string(), env.clone());
        }
        metadata.insert(
            "collect.mode".to_string(),
            self.collect.mode.as_str().to_string(),
        );
        metadata.insert("idempotency_key".to_string(), self.idempotency_key());
        if let Some(cursor) = self.collect.cursor.as_ref() {
            metadata.insert("collect.cursor".to_string(), cursor.clone());
        }
        if let Some(res_ver) = self.collect.res_ver.as_ref() {
            metadata.insert("collect.res_ver".to_string(), res_ver.clone());
        }

        IngestEnvelope {
            ingest_id,
            source_kind,
            source_name: format!("{}:{}", self.source.system, self.source.producer),
            ingest_mode,
            tenant_id,
            environment_id,
            observed_at,
            received_at,
            payload_ref: None,
            payload_inline: Some(self.payload),
            metadata,
        }
    }
}

fn require_non_empty(value: &str, field: &'static str) -> DomainResult<()> {
    if value.trim().is_empty() {
        Err(DomainReason::FieldInvalid
            .to_err()
            .with_detail(format!("{field} must not be empty")))
    } else {
        Ok(())
    }
}

fn require_option_non_empty(value: Option<&str>, field: &'static str) -> DomainResult<()> {
    match value {
        Some(value) => require_non_empty(value, field),
        None => Err(missing_field(field, format!("{field} is required"))),
    }
}

fn require_json_string(value: Option<&Value>, field: &'static str) -> DomainResult<()> {
    match value {
        Some(Value::String(value)) => require_non_empty(value, field),
        Some(_) => Err(DomainReason::FieldInvalid
            .to_err()
            .with_detail(format!("{field} must be a string"))),
        None => Err(missing_field(field, format!("{field} is required"))),
    }
}

fn invalid_schema(detail: impl Into<String>) -> DomainError {
    DomainReason::SchemaInvalid.to_err().with_detail(detail)
}

fn missing_field(_field: &'static str, detail: impl Into<String>) -> DomainError {
    DomainReason::FieldMissing.to_err().with_detail(detail)
}

fn deserialize_optional_string_like<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringLike {
        String(String),
        Integer(i64),
        Unsigned(u64),
        Float(f64),
        Bool(bool),
    }

    let value = Option::<StringLike>::deserialize(deserializer)?;
    Ok(value.map(|value| match value {
        StringLike::String(value) => value,
        StringLike::Integer(value) => value.to_string(),
        StringLike::Unsigned(value) => value.to_string(),
        StringLike::Float(value) => value.to_string(),
        StringLike::Bool(value) => value.to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use orion_error::reason::ErrorIdentityProvider;
    use serde_json::json;
    use uuid::Uuid;

    use crate::TenantId;

    use super::{DayuInputEnvelope, DayuInputMode};

    #[test]
    fn dayu_input_validate_accepts_target_snapshot() {
        let input: DayuInputEnvelope = serde_json::from_value(json!({
            "schema": "dayu.in.edge.v1",
            "source": {
                "system": "warp-insight",
                "producer": "agent-01",
                "tenant": "demo",
                "env": "prod"
            },
            "collect": {
                "mode": "snapshot",
                "snap_id": "snap-001",
                "observed_at": "2026-04-26T02:20:30Z"
            },
            "payload": {
                "hosts": []
            }
        }))
        .unwrap();

        input.validate().unwrap();
        assert_eq!(input.schema_family(), Some("edge"));
        assert!(matches!(input.collect.mode, DayuInputMode::Snapshot));
    }

    #[test]
    fn dayu_input_rejects_snapshot_without_snap_id() {
        let input: DayuInputEnvelope = serde_json::from_value(json!({
            "schema": "dayu.in.edge.v1",
            "source": {
                "system": "warp-insight",
                "producer": "agent-01",
                "tenant": "demo"
            },
            "collect": {
                "mode": "snapshot",
                "observed_at": "2026-04-26T02:20:30Z"
            },
            "payload": {}
        }))
        .unwrap();

        let err = input.validate().unwrap_err();
        assert_eq!(err.reason().stable_code(), "biz.dayu.domain.field_missing");
        assert!(
            err.detail()
                .as_deref()
                .is_some_and(|detail| detail.contains("collect.snap_id"))
        );
    }

    #[test]
    fn dayu_input_uses_standard_snapshot_idempotency_key_as_ingest_id() {
        let input: DayuInputEnvelope = serde_json::from_value(json!({
            "schema": "dayu.in.edge.v1",
            "source": {
                "system": "warp-insight",
                "producer": "agent-01",
                "tenant": "demo",
                "env": "prod"
            },
            "collect": {
                "mode": "snapshot",
                "snap_id": "snap-001",
                "observed_at": "2026-04-26T02:20:30Z"
            },
            "payload": {}
        }))
        .unwrap();

        let ingest = input.into_ingest_envelope(TenantId(Uuid::new_v4()), None, Utc::now());

        assert_eq!(
            ingest.ingest_id,
            "dayu.in.edge.v1:warp-insight:agent-01:demo:prod:snap-001"
        );
        assert_eq!(
            ingest.metadata.get("idempotency_key").map(String::as_str),
            Some("dayu.in.edge.v1:warp-insight:agent-01:demo:prod:snap-001")
        );
    }

    #[test]
    fn dayu_input_accepts_short_source_aliases_and_numeric_res_ver() {
        let input: DayuInputEnvelope = serde_json::from_value(json!({
            "schema": "dayu.in.edge.v1",
            "source": {
                "kind": "edge",
                "system": "warp-insight",
                "producer": "agent-local-01",
                "tenant_ref": "tenant-demo",
                "env_ref": "office"
            },
            "collect": {
                "mode": "snapshot",
                "snap_id": "edge-snap-local-01",
                "observed_at": "2026-05-12T03:16:04Z",
                "collected_at": "2026-05-12T03:16:05Z",
                "res_ver": 4
            },
            "payload": {
                "host_name": "local-host"
            }
        }))
        .unwrap();

        input.validate().unwrap();
        assert_eq!(input.source.tenant, "tenant-demo");
        assert_eq!(input.source.env.as_deref(), Some("office"));
        assert_eq!(input.collect.res_ver.as_deref(), Some("4"));
        assert_eq!(
            input.idempotency_key(),
            "dayu.in.edge.v1:warp-insight:agent-local-01:tenant-demo:office:edge-snap-local-01"
        );
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum IngestMode {
    Snapshot,
    Delta,
    BatchUpsert,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IngestEnvelope {
    pub ingest_id: String,
    pub source_kind: SourceKind,
    pub source_name: String,
    pub ingest_mode: IngestMode,
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
pub struct ProcessRuntimeCandidate {
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub source_kind: SourceKind,
    pub host_name: Option<String>,
    pub machine_id: Option<String>,
    pub pid: i32,
    pub executable: String,
    pub command_line: Option<String>,
    pub identity: Option<String>,
    pub service_ref: Option<String>,
    pub instance_key: Option<String>,
    pub observed_at: Option<ObservedAt>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkSegmentCandidate {
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub source_kind: SourceKind,
    pub segment_name: Option<String>,
    pub cidr: Option<String>,
    pub gateway_ip: Option<String>,
    pub ip_addr: Option<String>,
    pub host_name: Option<String>,
    pub machine_id: Option<String>,
    pub iface_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HostTelemetryCandidate {
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub source_kind: SourceKind,
    pub host_name: Option<String>,
    pub machine_id: Option<String>,
    pub observed_at: ObservedAt,
    pub metric_name: String,
    pub value_i64: Option<i64>,
    pub value_f64: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcessTelemetryCandidate {
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub source_kind: SourceKind,
    pub host_name: Option<String>,
    pub machine_id: Option<String>,
    pub process_ref: String,
    pub pid: i32,
    pub observed_at: ObservedAt,
    pub metric_name: String,
    pub value_i64: Option<i64>,
    pub value_string: Option<String>,
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
    pub subject_display_name: Option<String>,
    pub subject_external_ref: Option<String>,
    pub subject_email: Option<String>,
    pub target_kind: crate::ObjectKind,
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
