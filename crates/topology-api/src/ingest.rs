use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use orion_error::conversion::ConvErr;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use topology_domain::{
    BusinessCatalogCandidate, DayuInputEnvelope, HostCandidate, HostTelemetryCandidate,
    IngestEnvelope, IngestMode, NetworkSegmentCandidate, ProcessRuntimeCandidate,
    ProcessTelemetryCandidate, ResponsibilityAssignmentCandidate, ServiceBoundary, ServiceType,
    SourceKind, SubjectCandidate, SubjectType,
};

use crate::error::{
    ApiResult, invalid_field_type, invalid_field_value, missing_field, missing_payload,
    payload_must_be_object, recorder_unavailable, unsupported_ingest_mode,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum IngestJobStatus {
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IngestJobRecord {
    pub ingest_id: String,
    pub tenant_id: topology_domain::TenantId,
    pub source_kind: SourceKind,
    pub source_name: String,
    pub received_at: DateTime<Utc>,
    pub status: IngestJobStatus,
    pub payload_ref: Option<String>,
    pub error: Option<String>,
}

impl IngestJobRecord {
    pub fn accepted(envelope: &IngestEnvelope) -> Self {
        Self {
            ingest_id: envelope.ingest_id.clone(),
            tenant_id: envelope.tenant_id,
            source_kind: envelope.source_kind,
            source_name: envelope.source_name.clone(),
            received_at: envelope.received_at,
            status: IngestJobStatus::Accepted,
            payload_ref: envelope.payload_ref.clone(),
            error: None,
        }
    }
}

pub trait IngestJobRecorder {
    fn record_ingest_job(&self, record: IngestJobRecord) -> ApiResult<()>;
}

#[derive(Debug, Default, Clone)]
pub struct InMemoryIngestJobRecorder {
    records: Arc<Mutex<Vec<IngestJobRecord>>>,
}

impl InMemoryIngestJobRecorder {
    pub fn records(&self) -> ApiResult<Vec<IngestJobRecord>> {
        self.records
            .lock()
            .map(|records| records.clone())
            .map_err(|_| recorder_unavailable())
    }
}

impl IngestJobRecorder for InMemoryIngestJobRecorder {
    fn record_ingest_job(&self, record: IngestJobRecord) -> ApiResult<()> {
        self.records
            .lock()
            .map_err(|_| recorder_unavailable())?
            .push(record);
        Ok(())
    }
}

pub struct IngestService<R> {
    recorder: R,
}

impl<R> IngestService<R>
where
    R: IngestJobRecorder,
{
    pub fn new(recorder: R) -> Self {
        Self { recorder }
    }

    pub fn submit(&self, envelope: IngestEnvelope) -> ApiResult<IngestJobRecord> {
        if envelope.payload_inline.is_none() && envelope.payload_ref.is_none() {
            return Err(missing_payload());
        }

        if envelope.ingest_mode == IngestMode::Delta {
            return Err(unsupported_ingest_mode());
        }

        let record = IngestJobRecord::accepted(&envelope);
        self.recorder.record_ingest_job(record.clone())?;
        Ok(record)
    }

    pub fn submit_dayu_input(
        &self,
        input: DayuInputEnvelope,
        tenant_id: topology_domain::TenantId,
        environment_id: Option<topology_domain::EnvironmentId>,
    ) -> ApiResult<IngestJobRecord> {
        input.validate().conv_err()?;
        self.submit(input.into_ingest_envelope(tenant_id, environment_id, Utc::now()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractedBusinessCatalog {
    pub candidates: Vec<BusinessCatalogCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractedHosts {
    pub candidates: Vec<HostCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractedNetworkSegments {
    pub candidates: Vec<NetworkSegmentCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractedProcessRuntimes {
    pub candidates: Vec<ProcessRuntimeCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtractedHostTelemetry {
    pub candidates: Vec<HostTelemetryCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtractedProcessTelemetry {
    pub candidates: Vec<ProcessTelemetryCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractedSubjects {
    pub candidates: Vec<SubjectCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractedResponsibilityAssignments {
    pub candidates: Vec<ResponsibilityAssignmentCandidate>,
}

pub fn extract_business_catalog_candidates(
    envelope: &IngestEnvelope,
) -> ApiResult<ExtractedBusinessCatalog> {
    let payload = object_payload(envelope)?;
    let rows = first_array(payload, "business_catalog", "items")?;

    let mut candidates = Vec::with_capacity(rows.len());
    for row in rows {
        let row = row
            .as_object()
            .ok_or(invalid_field_type("business_catalog[]"))?;
        let business_name = required_string(row, "business_name")?;
        let service_name = optional_string(row, "service_name")?;

        candidates.push(BusinessCatalogCandidate {
            tenant_id: envelope.tenant_id,
            source_kind: envelope.source_kind,
            external_ref: optional_string(row, "external_ref")?,
            business_name,
            system_name: optional_string(row, "system_name")?,
            subsystem_name: optional_string(row, "subsystem_name")?,
            service_name,
            service_type: optional_service_type(row, "service_type")?,
            boundary: optional_service_boundary(row, "boundary")?,
        });
    }

    Ok(ExtractedBusinessCatalog { candidates })
}

pub fn extract_host_candidates(envelope: &IngestEnvelope) -> ApiResult<ExtractedHosts> {
    let payload = object_payload(envelope)?;
    let mut candidates = Vec::new();
    if looks_like_edge_host_fact(payload)? {
        candidates.push(host_candidate_from_row(envelope, payload)?);
    }

    let rows = if payload.contains_key("hosts") || payload.contains_key("items") {
        first_array(payload, "hosts", "items")?
    } else {
        Vec::new()
    };

    candidates.reserve(rows.len());
    for row in rows {
        let row = row.as_object().ok_or(invalid_field_type("hosts[]"))?;
        candidates.push(host_candidate_from_row(envelope, row)?);
    }

    Ok(ExtractedHosts { candidates })
}

fn looks_like_edge_host_fact(object: &serde_json::Map<String, Value>) -> ApiResult<bool> {
    let target_kind = optional_string(object, "target_kind")?;
    if let Some(target_kind) = target_kind.as_deref() {
        return Ok(target_kind == "host");
    }

    Ok(optional_string(object, "host_name")?.is_some()
        || optional_string(object, "hostname")?.is_some())
}

fn host_candidate_from_row(
    envelope: &IngestEnvelope,
    row: &serde_json::Map<String, Value>,
) -> ApiResult<HostCandidate> {
    let host_name = required_string_any(row, &["host_name", "hostname"])?;
    let os = row.get("os").and_then(Value::as_object);

    Ok(HostCandidate {
        tenant_id: envelope.tenant_id,
        environment_id: envelope.environment_id,
        source_kind: envelope.source_kind,
        external_ref: optional_string(row, "external_ref")?,
        host_name,
        machine_id: optional_string(row, "machine_id")?,
        os_name: optional_string(row, "os_name")?
            .or_else(|| os.and_then(|os| optional_string_lossy(os, "name"))),
        os_version: optional_string(row, "os_version")?
            .or_else(|| os.and_then(|os| optional_string_lossy(os, "version"))),
    })
}

pub fn extract_network_segment_candidates(
    envelope: &IngestEnvelope,
) -> ApiResult<ExtractedNetworkSegments> {
    let payload = object_payload(envelope)?;
    if let Some(interface_rows) = optional_array(payload, "interfaces")? {
        return extract_network_segments_from_interfaces(envelope, interface_rows);
    }

    if looks_like_edge_network_fact(payload)? {
        return Ok(ExtractedNetworkSegments {
            candidates: vec![network_candidate_from_row(envelope, payload)?],
        });
    }

    let rows = if payload.contains_key("network_segments") {
        first_array(payload, "network_segments", "items")?
    } else {
        first_array(payload, "ip_addresses", "ips")?
    };

    let mut candidates = Vec::with_capacity(rows.len());
    for row in rows {
        let row = row
            .as_object()
            .ok_or(invalid_field_type("network_segments[]"))?;

        let cidr = optional_string(row, "cidr")?;
        let ip_addr = match optional_string(row, "ip_addr")? {
            Some(value) => Some(value),
            None => optional_string(row, "ip")?,
        };

        if cidr.is_none() && ip_addr.is_none() {
            return Err(missing_field("cidr/ip_addr"));
        }

        let segment_name = match optional_string(row, "segment_name")? {
            Some(value) => Some(value),
            None => optional_string(row, "name")?,
        };

        candidates.push(NetworkSegmentCandidate {
            tenant_id: envelope.tenant_id,
            environment_id: envelope.environment_id,
            source_kind: envelope.source_kind,
            segment_name,
            cidr,
            gateway_ip: optional_string(row, "gateway_ip")?,
            ip_addr,
            host_name: optional_string(row, "host_name")?,
            machine_id: optional_string(row, "machine_id")?,
            iface_name: optional_string(row, "iface_name")?,
        });
    }

    Ok(ExtractedNetworkSegments { candidates })
}

pub fn extract_process_runtime_candidates(
    envelope: &IngestEnvelope,
) -> ApiResult<ExtractedProcessRuntimes> {
    let payload = object_payload(envelope)?;
    if !looks_like_edge_process_fact(payload)? {
        return Ok(ExtractedProcessRuntimes {
            candidates: Vec::new(),
        });
    }

    Ok(ExtractedProcessRuntimes {
        candidates: vec![process_candidate_from_row(envelope, payload)?],
    })
}

fn looks_like_edge_network_fact(object: &serde_json::Map<String, Value>) -> ApiResult<bool> {
    let target_kind = optional_string(object, "target_kind")?;
    if let Some(target_kind) = target_kind.as_deref() {
        if matches!(target_kind, "host" | "process" | "container") {
            return Ok(false);
        }
        if matches!(
            target_kind,
            "network_interface" | "host_network" | "network" | "ip" | "ip_address"
        ) {
            return Ok(true);
        }
    }

    Ok(optional_string(object, "ip")?.is_some()
        || optional_string(object, "ip_addr")?.is_some()
        || optional_string(object, "cidr")?.is_some())
}

fn network_candidate_from_row(
    envelope: &IngestEnvelope,
    row: &serde_json::Map<String, Value>,
) -> ApiResult<NetworkSegmentCandidate> {
    let ip_addr = match optional_string(row, "ip_addr")? {
        Some(value) => Some(value),
        None => optional_string(row, "ip")?,
    };
    let prefix = optional_u64(row, "prefix")?;
    let cidr = optional_string(row, "cidr")?.or_else(|| {
        ip_addr
            .as_ref()
            .zip(prefix)
            .map(|(ip, prefix)| network_cidr(ip, prefix))
    });

    if cidr.is_none() && ip_addr.is_none() {
        return Err(missing_field("cidr/ip_addr"));
    }

    let segment_name = match optional_string(row, "segment_name")? {
        Some(value) => Some(value),
        None => optional_string(row, "name")?,
    }
    .or_else(|| cidr.clone())
    .or_else(|| ip_addr.clone());

    Ok(NetworkSegmentCandidate {
        tenant_id: envelope.tenant_id,
        environment_id: envelope.environment_id,
        source_kind: envelope.source_kind,
        segment_name,
        cidr,
        gateway_ip: optional_string(row, "gateway_ip")?
            .or_else(|| optional_string_lossy(row, "gateway")),
        ip_addr,
        host_name: optional_string(row, "host_name")?
            .or_else(|| optional_string_lossy(row, "hostname")),
        machine_id: optional_string(row, "machine_id")?,
        iface_name: optional_string(row, "iface_name")?
            .or_else(|| optional_string_lossy(row, "interface_name"))
            .or_else(|| optional_string_lossy(row, "name")),
    })
}

fn looks_like_edge_process_fact(object: &serde_json::Map<String, Value>) -> ApiResult<bool> {
    let target_kind = optional_string(object, "target_kind")?;
    if let Some(target_kind) = target_kind.as_deref() {
        return Ok(target_kind == "process");
    }

    Ok(optional_string(object, "pid")?.is_some()
        && optional_string(object, "process_key")?.is_some())
}

fn process_candidate_from_row(
    envelope: &IngestEnvelope,
    row: &serde_json::Map<String, Value>,
) -> ApiResult<ProcessRuntimeCandidate> {
    let pid_raw = required_string(row, "pid")?;
    let pid = pid_raw
        .parse::<i32>()
        .map_err(|_| invalid_field_value("pid", pid_raw.clone()))?;
    let executable = optional_string(row, "executable")?
        .or_else(|| optional_string_lossy(row, "executable_name"))
        .or_else(|| optional_string_lossy(row, "process_key"))
        .ok_or_else(|| missing_field("executable_name"))?;
    let observed_at = optional_timestamp(row, "observed_at")?;
    let host_name = optional_string(row, "host_name")?
        .or_else(|| optional_string_lossy(row, "hostname"))
        .or_else(|| process_host_locator(row).and_then(|locator| locator.1));
    let machine_id = optional_string(row, "machine_id")?
        .or_else(|| process_host_locator(row).and_then(|locator| Some(locator.0)));

    Ok(ProcessRuntimeCandidate {
        tenant_id: envelope.tenant_id,
        environment_id: envelope.environment_id,
        source_kind: envelope.source_kind,
        host_name,
        machine_id,
        pid,
        executable,
        command_line: optional_string(row, "command_line")?,
        identity: optional_string(row, "identity")?,
        service_ref: optional_string(row, "service_ref")?,
        instance_key: optional_string(row, "instance_key")?,
        observed_at,
    })
}

fn process_host_locator(row: &serde_json::Map<String, Value>) -> Option<(String, Option<String>)> {
    for key in ["machine_id", "target_ref", "process_key", "external_ref"] {
        let Some(value) = optional_string_lossy(row, key) else {
            continue;
        };
        if let Some(machine_id) = extract_machine_id_prefix(&value) {
            let host_name = machine_id.strip_prefix("hostname:").map(str::to_string);
            return Some((machine_id, host_name));
        }
    }

    None
}

fn extract_machine_id_prefix(value: &str) -> Option<String> {
    if let Some(prefix) = value.strip_suffix(":host") {
        if prefix.starts_with("hostname:") && prefix.len() > "hostname:".len() {
            return Some(prefix.to_string());
        }
    }

    if let Some(prefix) = value.strip_suffix(":process") {
        if prefix.starts_with("hostname:") && prefix.len() > "hostname:".len() {
            return Some(prefix.to_string());
        }
    }

    let prefix = value.split(":pid:").next()?;
    if prefix.starts_with("hostname:") && prefix.len() > "hostname:".len() {
        return Some(prefix.to_string());
    }
    None
}

pub fn extract_subject_candidates(envelope: &IngestEnvelope) -> ApiResult<ExtractedSubjects> {
    let payload = object_payload(envelope)?;
    let mut candidates = Vec::new();

    for row in first_array(payload, "subjects", "items")? {
        candidates.push(subject_candidate_from_row(
            envelope,
            row,
            "subjects[]",
            SubjectType::User,
        )?);
    }

    if let Some(rows) = optional_array(payload, "users")? {
        for row in rows {
            candidates.push(subject_candidate_from_row(
                envelope,
                row,
                "users[]",
                SubjectType::User,
            )?);
        }
    }

    if let Some(rows) = optional_array(payload, "groups")? {
        for row in rows {
            candidates.push(subject_candidate_from_row(
                envelope,
                row,
                "groups[]",
                SubjectType::Team,
            )?);
        }
    }

    Ok(ExtractedSubjects { candidates })
}

pub fn extract_host_telemetry_candidates(
    envelope: &IngestEnvelope,
) -> ApiResult<ExtractedHostTelemetry> {
    let payload = object_payload(envelope)?;
    let metric_name = optional_string(payload, "metric_name")?;
    let collection_kind = optional_string(payload, "collection_kind")?;
    let target_ref = optional_string(payload, "resource_ref")?
        .or_else(|| optional_string_lossy(payload, "target_ref"));

    if collection_kind.as_deref() != Some("host_metrics") || metric_name.is_none() {
        return Ok(ExtractedHostTelemetry {
            candidates: Vec::new(),
        });
    }

    let observed_at = envelope
        .observed_at
        .ok_or_else(|| missing_field("collect.observed_at"))?;
    let metric_name = metric_name.expect("checked above");
    let locator = target_ref.as_deref().and_then(extract_machine_id_prefix);
    let machine_id = optional_string(payload, "machine_id")?.or(locator.clone());
    let host_name = optional_string(payload, "host_name")?
        .or_else(|| optional_string_lossy(payload, "hostname"))
        .or_else(|| locator.and_then(|item| item.strip_prefix("hostname:").map(str::to_string)));

    Ok(ExtractedHostTelemetry {
        candidates: vec![HostTelemetryCandidate {
            tenant_id: envelope.tenant_id,
            environment_id: envelope.environment_id,
            source_kind: envelope.source_kind,
            host_name,
            machine_id,
            observed_at,
            metric_name,
            value_i64: metric_value_i64(payload),
            value_f64: metric_value_f64(payload),
        }],
    })
}

pub fn extract_process_telemetry_candidates(
    envelope: &IngestEnvelope,
) -> ApiResult<ExtractedProcessTelemetry> {
    let payload = object_payload(envelope)?;
    let metric_name = optional_string(payload, "metric_name")?;
    let collection_kind = optional_string(payload, "collection_kind")?;
    let process_ref = optional_string(payload, "resource_ref")?
        .or_else(|| optional_string_lossy(payload, "target_ref"));

    if collection_kind.as_deref() != Some("process_metrics")
        || metric_name.is_none()
        || process_ref.is_none()
    {
        return Ok(ExtractedProcessTelemetry {
            candidates: Vec::new(),
        });
    }

    let observed_at = envelope
        .observed_at
        .ok_or_else(|| missing_field("collect.observed_at"))?;
    let process_ref = process_ref.expect("checked above");
    let process_locator = extract_process_ref_parts(&process_ref)
        .ok_or_else(|| invalid_field_value("payload.resource_ref", process_ref.clone()))?;

    Ok(ExtractedProcessTelemetry {
        candidates: vec![ProcessTelemetryCandidate {
            tenant_id: envelope.tenant_id,
            environment_id: envelope.environment_id,
            source_kind: envelope.source_kind,
            host_name: process_locator
                .machine_id
                .strip_prefix("hostname:")
                .map(str::to_string),
            machine_id: Some(process_locator.machine_id),
            process_ref,
            pid: process_locator.pid,
            observed_at,
            metric_name: metric_name.expect("checked above"),
            value_i64: metric_value_i64(payload),
            value_string: metric_value_string(payload),
        }],
    })
}

pub fn extract_responsibility_assignment_candidates(
    envelope: &IngestEnvelope,
) -> ApiResult<ExtractedResponsibilityAssignments> {
    let payload = object_payload(envelope)?;
    let rows = first_array(payload, "responsibility_assignments", "items")?;

    let mut candidates = Vec::with_capacity(rows.len());
    for row in rows {
        let row = row
            .as_object()
            .ok_or(invalid_field_type("responsibility_assignments[]"))?;

        candidates.push(ResponsibilityAssignmentCandidate {
            tenant_id: envelope.tenant_id,
            source_kind: envelope.source_kind,
            subject_display_name: optional_string(row, "subject_display_name")?,
            subject_external_ref: optional_string(row, "subject_external_ref")?,
            subject_email: optional_string(row, "subject_email")?,
            target_kind: optional_object_kind(row, "target_kind")?
                .unwrap_or(topology_domain::ObjectKind::Host),
            target_external_ref: optional_string(row, "target_external_ref")?,
            role: optional_responsibility_role(row, "role")?
                .unwrap_or(topology_domain::ResponsibilityRole::Owner),
            validity: topology_domain::ValidityWindow {
                valid_from: envelope.received_at,
                valid_to: None,
            },
        });
    }

    Ok(ExtractedResponsibilityAssignments { candidates })
}

fn object_payload(envelope: &IngestEnvelope) -> ApiResult<&serde_json::Map<String, Value>> {
    envelope
        .payload_inline
        .as_ref()
        .ok_or(missing_payload())?
        .as_object()
        .ok_or(payload_must_be_object())
}

fn optional_array<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<Vec<&'a Value>>> {
    match object.get(field) {
        Some(Value::Array(values)) => Ok(Some(values.iter().collect())),
        Some(_) => Err(invalid_field_type(field)),
        None => Ok(None),
    }
}

fn first_array<'a>(
    object: &'a serde_json::Map<String, Value>,
    preferred: &'static str,
    fallback: &'static str,
) -> ApiResult<Vec<&'a Value>> {
    if let Some(values) = optional_array(object, preferred)? {
        return Ok(values);
    }

    Ok(optional_array(object, fallback)?.unwrap_or_default())
}

fn required_string(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<String> {
    optional_string(object, field)?.ok_or(missing_field(field))
}

fn required_string_any(
    object: &serde_json::Map<String, Value>,
    fields: &'static [&'static str],
) -> ApiResult<String> {
    for field in fields {
        if let Some(value) = optional_string(object, field)? {
            return Ok(value);
        }
    }

    Err(missing_field(fields[0]))
}

fn optional_string(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<String>> {
    match object.get(field) {
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(Some(value.clone())),
        Some(Value::String(_)) => Ok(None),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(invalid_field_type(field)),
    }
}

fn optional_string_lossy(object: &serde_json::Map<String, Value>, field: &str) -> Option<String> {
    match object.get(field) {
        Some(Value::String(value)) if !value.trim().is_empty() => Some(value.clone()),
        _ => None,
    }
}

fn metric_value_i64(object: &serde_json::Map<String, Value>) -> Option<i64> {
    match object.get("value") {
        Some(Value::Number(value)) => value.as_i64(),
        _ => None,
    }
}

fn metric_value_f64(object: &serde_json::Map<String, Value>) -> Option<f64> {
    match object.get("value") {
        Some(Value::Number(value)) => value.as_f64(),
        _ => None,
    }
}

fn metric_value_string(object: &serde_json::Map<String, Value>) -> Option<String> {
    match object.get("value") {
        Some(Value::String(value)) if !value.trim().is_empty() => Some(value.clone()),
        _ => None,
    }
}

struct ProcessRefParts {
    machine_id: String,
    pid: i32,
}

fn extract_process_ref_parts(value: &str) -> Option<ProcessRefParts> {
    let machine_id = extract_machine_id_prefix(value)?;
    let pid_part = value.split(":pid:").nth(1)?;
    let pid_raw = pid_part.split(':').next()?;
    let pid = pid_raw.parse::<i32>().ok()?;

    Some(ProcessRefParts { machine_id, pid })
}

fn extract_network_segments_from_interfaces(
    envelope: &IngestEnvelope,
    interface_rows: Vec<&Value>,
) -> ApiResult<ExtractedNetworkSegments> {
    let payload = object_payload(envelope)?;
    let host = payload.get("host").and_then(Value::as_object);
    let host_from_hosts = payload
        .get("hosts")
        .and_then(Value::as_array)
        .and_then(|hosts| hosts.first())
        .and_then(Value::as_object);
    let host_name = host
        .and_then(|host| optional_string_lossy(host, "host_name"))
        .or_else(|| host.and_then(|host| optional_string_lossy(host, "hostname")))
        .or_else(|| host_from_hosts.and_then(|host| optional_string_lossy(host, "host_name")))
        .or_else(|| host_from_hosts.and_then(|host| optional_string_lossy(host, "hostname")));
    let machine_id = host
        .and_then(|host| optional_string_lossy(host, "machine_id"))
        .or_else(|| host_from_hosts.and_then(|host| optional_string_lossy(host, "machine_id")));

    let mut candidates = Vec::new();
    for row in interface_rows {
        let row = row.as_object().ok_or(invalid_field_type("interfaces[]"))?;
        let iface_name = optional_string(row, "name")?;
        let row_host_ref = optional_string(row, "host_ref")?;
        let row_machine_id = optional_string(row, "machine_id")?;
        let addresses = optional_array(row, "addresses")?.unwrap_or_default();
        for address in addresses {
            let address = address
                .as_object()
                .ok_or(invalid_field_type("interfaces[].addresses[]"))?;
            let ip_addr = optional_string(address, "ip")?;
            let prefix = optional_u64(address, "prefix")?;
            let cidr = optional_string(address, "cidr")?.or_else(|| {
                ip_addr
                    .as_ref()
                    .zip(prefix)
                    .map(|(ip, prefix)| network_cidr(ip, prefix))
            });
            candidates.push(NetworkSegmentCandidate {
                tenant_id: envelope.tenant_id,
                environment_id: envelope.environment_id,
                source_kind: envelope.source_kind,
                segment_name: cidr.clone().or_else(|| ip_addr.clone()),
                cidr,
                gateway_ip: optional_string(address, "gateway")?,
                ip_addr,
                host_name: row_host_ref.clone().or_else(|| host_name.clone()),
                machine_id: row_machine_id.clone().or_else(|| machine_id.clone()),
                iface_name: iface_name.clone(),
            });
        }
    }

    Ok(ExtractedNetworkSegments { candidates })
}

fn network_cidr(ip: &str, prefix: u64) -> String {
    if prefix <= 32 {
        if let Ok(ipv4) = ip.parse::<std::net::Ipv4Addr>() {
            let ip_num = u32::from(ipv4);
            let mask = if prefix == 0 {
                0
            } else {
                u32::MAX << (32 - prefix)
            };
            return format!("{}/{}", std::net::Ipv4Addr::from(ip_num & mask), prefix);
        }
    }

    format!("{ip}/{prefix}")
}

fn subject_candidate_from_row(
    envelope: &IngestEnvelope,
    row: &Value,
    field: &'static str,
    default_type: SubjectType,
) -> ApiResult<SubjectCandidate> {
    let row = row.as_object().ok_or(invalid_field_type(field))?;
    let display_name = required_string_any(row, &["display_name", "name"])?;
    let subject_type = match optional_subject_type(row, "subject_type")? {
        Some(subject_type) => Some(subject_type),
        None => optional_subject_type(row, "group_type")?,
    }
    .unwrap_or(default_type);

    Ok(SubjectCandidate {
        tenant_id: envelope.tenant_id,
        source_kind: envelope.source_kind,
        subject_type,
        external_ref: optional_string(row, "external_ref")?
            .or_else(|| optional_string_lossy(row, "external_id")),
        display_name,
        email: optional_string(row, "email")?,
        is_active: optional_bool(row, "is_active")?.unwrap_or(true),
    })
}

fn optional_u64(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<u64>> {
    match object.get(field) {
        Some(Value::Number(value)) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| invalid_field_type(field)),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(invalid_field_type(field)),
    }
}

fn optional_timestamp(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<topology_domain::ObservedAt>> {
    match object.get(field) {
        Some(Value::String(value)) => {
            let parsed = chrono::DateTime::parse_from_rfc3339(value)
                .map_err(|_| invalid_field_value(field, value.clone()))?
                .with_timezone(&Utc);
            Ok(Some(topology_domain::ObservedAt(parsed)))
        }
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(invalid_field_type(field)),
    }
}

fn optional_bool(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<bool>> {
    match object.get(field) {
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(invalid_field_type(field)),
    }
}

fn optional_service_type(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<ServiceType>> {
    optional_string(object, field)?
        .map(|value| match value.as_str() {
            "application" => Ok(ServiceType::Application),
            "data" => Ok(ServiceType::Data),
            "platform" => Ok(ServiceType::Platform),
            "shared" => Ok(ServiceType::Shared),
            _ => Err(invalid_field_value(field, value)),
        })
        .transpose()
}

fn optional_service_boundary(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<ServiceBoundary>> {
    optional_string(object, field)?
        .map(|value| match value.as_str() {
            "internal" => Ok(ServiceBoundary::Internal),
            "external" => Ok(ServiceBoundary::External),
            "partner" => Ok(ServiceBoundary::Partner),
            "saas" => Ok(ServiceBoundary::Saas),
            _ => Err(invalid_field_value(field, value)),
        })
        .transpose()
}

fn optional_subject_type(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<SubjectType>> {
    optional_string(object, field)?
        .map(|value| match value.as_str() {
            "user" => Ok(SubjectType::User),
            "team" => Ok(SubjectType::Team),
            "rotation" => Ok(SubjectType::Rotation),
            "service_account" => Ok(SubjectType::ServiceAccount),
            _ => Err(invalid_field_value(field, value)),
        })
        .transpose()
}

fn optional_responsibility_role(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<topology_domain::ResponsibilityRole>> {
    optional_string(object, field)?
        .map(|value| match value.as_str() {
            "owner" => Ok(topology_domain::ResponsibilityRole::Owner),
            "maintainer" => Ok(topology_domain::ResponsibilityRole::Maintainer),
            "oncall" => Ok(topology_domain::ResponsibilityRole::Oncall),
            "security" => Ok(topology_domain::ResponsibilityRole::Security),
            _ => Err(invalid_field_value(field, value)),
        })
        .transpose()
}

fn optional_object_kind(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<topology_domain::ObjectKind>> {
    optional_string(object, field)?
        .map(|value| match value.as_str() {
            "host" => Ok(topology_domain::ObjectKind::Host),
            "network_segment" => Ok(topology_domain::ObjectKind::NetworkSegment),
            "subject" => Ok(topology_domain::ObjectKind::Subject),
            _ => Err(invalid_field_value(field, value)),
        })
        .transpose()
}

#[allow(dead_code)]
fn empty_metadata() -> BTreeMap<String, String> {
    BTreeMap::new()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use orion_error::reason::ErrorIdentityProvider;
    use serde_json::json;
    use topology_domain::{IngestEnvelope, IngestMode, SourceKind, TenantId};
    use uuid::Uuid;

    use super::{
        InMemoryIngestJobRecorder, IngestJobStatus, IngestService,
        extract_business_catalog_candidates, extract_host_candidates,
        extract_host_telemetry_candidates, extract_network_segment_candidates,
        extract_process_runtime_candidates, extract_process_telemetry_candidates,
        extract_responsibility_assignment_candidates, extract_subject_candidates,
    };

    fn envelope(payload_inline: serde_json::Value) -> IngestEnvelope {
        IngestEnvelope {
            ingest_id: "ing-1".to_string(),
            source_kind: SourceKind::BatchImport,
            source_name: "test".to_string(),
            ingest_mode: IngestMode::Snapshot,
            tenant_id: TenantId(Uuid::new_v4()),
            environment_id: None,
            observed_at: None,
            received_at: Utc::now(),
            payload_ref: None,
            payload_inline: Some(payload_inline),
            metadata: Default::default(),
        }
    }

    #[test]
    fn submit_records_ingest_job() {
        let recorder = InMemoryIngestJobRecorder::default();
        let service = IngestService::new(recorder.clone());
        let record = service.submit(envelope(json!({ "items": [] }))).unwrap();

        assert_eq!(record.status, IngestJobStatus::Accepted);
        assert_eq!(recorder.records().unwrap().len(), 1);
    }

    #[test]
    fn submit_rejects_delta_until_supported() {
        let recorder = InMemoryIngestJobRecorder::default();
        let service = IngestService::new(recorder);
        let mut env = envelope(json!({ "items": [] }));
        env.ingest_mode = IngestMode::Delta;

        let err = service.submit(env).unwrap_err();
        assert_eq!(
            err.reason().stable_code(),
            "biz.dayu.api.ingest_mode_unsupported"
        );
    }

    #[test]
    fn extract_business_catalog_from_items() {
        let extracted = extract_business_catalog_candidates(&envelope(json!({
            "items": [{
                "external_ref": "svc-1",
                "business_name": "payments",
                "system_name": "checkout",
                "service_name": "billing",
                "service_type": "application",
                "boundary": "internal"
            }]
        })))
        .unwrap();

        let candidate = &extracted.candidates[0];
        assert_eq!(candidate.business_name, "payments");
        assert_eq!(candidate.service_name.as_deref(), Some("billing"));
    }

    #[test]
    fn extract_hosts_from_hosts_field() {
        let extracted = extract_host_candidates(&envelope(json!({
            "hosts": [{
                "external_ref": "host-1",
                "host_name": "node-01",
                "machine_id": "machine-01",
                "os_name": "linux"
            }]
        })))
        .unwrap();

        let candidate = &extracted.candidates[0];
        assert_eq!(candidate.host_name, "node-01");
        assert_eq!(candidate.machine_id.as_deref(), Some("machine-01"));
    }

    #[test]
    fn extract_hosts_from_target_edge_rows() {
        let extracted = extract_host_candidates(&envelope(json!({
            "hosts": [{
                "hostname": "node-01",
                "machine_id": "machine-01",
                "os": {
                    "name": "linux",
                    "version": "6.8.0"
                }
            }]
        })))
        .unwrap();

        let candidate = &extracted.candidates[0];
        assert_eq!(candidate.host_name, "node-01");
        assert_eq!(candidate.os_name.as_deref(), Some("linux"));
        assert_eq!(candidate.os_version.as_deref(), Some("6.8.0"));
    }

    #[test]
    fn extract_hosts_from_edge_host_fact_payload() {
        let extracted = extract_host_candidates(&envelope(json!({
            "target_kind": "host",
            "target_ref": "hostname:node-05",
            "external_ref": "hostname:node-05",
            "host_name": "node-05",
            "machine_id": "hostname:node-05"
        })))
        .unwrap();

        let candidate = &extracted.candidates[0];
        assert_eq!(candidate.host_name, "node-05");
        assert_eq!(candidate.machine_id.as_deref(), Some("hostname:node-05"));
        assert_eq!(candidate.external_ref.as_deref(), Some("hostname:node-05"));
    }

    #[test]
    fn extract_hosts_does_not_treat_edge_process_fact_as_host() {
        let extracted = extract_host_candidates(&envelope(json!({
            "target_kind": "process",
            "target_ref": "hostname:node-05:pid:123",
            "external_ref": "hostname:node-05:pid:123",
            "pid": "123"
        })))
        .unwrap();

        assert!(extracted.candidates.is_empty());
    }

    #[test]
    fn extract_network_segments_from_segment_rows() {
        let extracted = extract_network_segment_candidates(&envelope(json!({
            "network_segments": [{
                "segment_name": "office-lan",
                "cidr": "192.168.10.0/24",
                "gateway_ip": "192.168.10.1"
            }]
        })))
        .unwrap();

        let candidate = &extracted.candidates[0];
        assert_eq!(candidate.segment_name.as_deref(), Some("office-lan"));
        assert_eq!(candidate.cidr.as_deref(), Some("192.168.10.0/24"));
        assert_eq!(candidate.ip_addr, None);
    }

    #[test]
    fn extract_network_segments_from_ip_rows() {
        let extracted = extract_network_segment_candidates(&envelope(json!({
            "ips": [{
                "ip": "10.0.0.12",
                "iface_name": "eth0"
            }]
        })))
        .unwrap();

        let candidate = &extracted.candidates[0];
        assert_eq!(candidate.ip_addr.as_deref(), Some("10.0.0.12"));
        assert_eq!(candidate.iface_name.as_deref(), Some("eth0"));
        assert_eq!(candidate.cidr, None);
    }

    #[test]
    fn extract_network_segments_from_target_interfaces() {
        let extracted = extract_network_segment_candidates(&envelope(json!({
            "hosts": [{
                "hostname": "node-01",
                "machine_id": "machine-01"
            }],
            "interfaces": [{
                "host_ref": "node-01",
                "name": "eth0",
                "addresses": [{
                    "family": "ipv4",
                    "ip": "192.168.10.52",
                    "prefix": 24,
                    "gateway": "192.168.10.1"
                }]
            }]
        })))
        .unwrap();

        let candidate = &extracted.candidates[0];
        assert_eq!(candidate.host_name.as_deref(), Some("node-01"));
        assert_eq!(candidate.iface_name.as_deref(), Some("eth0"));
        assert_eq!(candidate.ip_addr.as_deref(), Some("192.168.10.52"));
        assert_eq!(candidate.cidr.as_deref(), Some("192.168.10.0/24"));
        assert_eq!(candidate.segment_name.as_deref(), Some("192.168.10.0/24"));
    }

    #[test]
    fn extract_network_segments_from_edge_network_fact_payload() {
        let extracted = extract_network_segment_candidates(&envelope(json!({
            "target_kind": "network_interface",
            "host_name": "node-06",
            "machine_id": "hostname:node-06",
            "iface_name": "eth0",
            "ip": "192.168.20.15",
            "prefix": 24,
            "gateway": "192.168.20.1"
        })))
        .unwrap();

        let candidate = &extracted.candidates[0];
        assert_eq!(candidate.host_name.as_deref(), Some("node-06"));
        assert_eq!(candidate.machine_id.as_deref(), Some("hostname:node-06"));
        assert_eq!(candidate.iface_name.as_deref(), Some("eth0"));
        assert_eq!(candidate.ip_addr.as_deref(), Some("192.168.20.15"));
        assert_eq!(candidate.cidr.as_deref(), Some("192.168.20.0/24"));
        assert_eq!(candidate.gateway_ip.as_deref(), Some("192.168.20.1"));
        assert_eq!(candidate.segment_name.as_deref(), Some("192.168.20.0/24"));
    }

    #[test]
    fn extract_network_segments_does_not_treat_edge_process_fact_as_network() {
        let extracted = extract_network_segment_candidates(&envelope(json!({
            "target_kind": "process",
            "pid": "123",
            "external_ref": "hostname:node-06:pid:123"
        })))
        .unwrap();

        assert!(extracted.candidates.is_empty());
    }

    #[test]
    fn extract_process_runtime_from_edge_process_fact_payload() {
        let extracted = extract_process_runtime_candidates(&envelope(json!({
            "target_kind": "process",
            "host_name": "node-08",
            "machine_id": "hostname:node-08",
            "pid": "231",
            "identity": "ps_lstart:Tue May 12 05:38:01 2026",
            "process_key": "hostname:node-08:pid:231:ps_lstart:Tue May 12 05:38:01 2026",
            "executable_name": "/usr/sbin/sshd",
            "observed_at": "2026-05-12T03:16:03Z"
        })))
        .unwrap();

        let candidate = &extracted.candidates[0];
        assert_eq!(candidate.host_name.as_deref(), Some("node-08"));
        assert_eq!(candidate.machine_id.as_deref(), Some("hostname:node-08"));
        assert_eq!(candidate.pid, 231);
        assert_eq!(candidate.executable, "/usr/sbin/sshd");
        assert_eq!(
            candidate.identity.as_deref(),
            Some("ps_lstart:Tue May 12 05:38:01 2026")
        );
        assert_eq!(candidate.service_ref, None);
        assert!(candidate.observed_at.is_some());
    }

    #[test]
    fn extract_process_runtime_derives_host_locator_from_process_key() {
        let extracted = extract_process_runtime_candidates(&envelope(json!({
            "target_kind": "process",
            "pid": "231",
            "identity": "ps_lstart:Tue May 12 05:38:01 2026",
            "process_key": "hostname:node-08:pid:231:ps_lstart:Tue May 12 05:38:01 2026",
            "executable_name": "/usr/sbin/sshd",
            "observed_at": "2026-05-12T03:16:03Z"
        })))
        .unwrap();

        let candidate = &extracted.candidates[0];
        assert_eq!(candidate.host_name.as_deref(), Some("node-08"));
        assert_eq!(candidate.machine_id.as_deref(), Some("hostname:node-08"));
        assert_eq!(candidate.pid, 231);
    }

    #[test]
    fn extract_process_runtime_does_not_treat_host_fact_as_process() {
        let extracted = extract_process_runtime_candidates(&envelope(json!({
            "target_kind": "host",
            "host_name": "node-08",
            "machine_id": "hostname:node-08"
        })))
        .unwrap();

        assert!(extracted.candidates.is_empty());
    }

    #[test]
    fn extract_host_telemetry_from_dayu_telemetry_payload() {
        let extracted = extract_host_telemetry_candidates(&IngestEnvelope {
            ingest_id: "telemetry-1".to_string(),
            source_kind: SourceKind::TelemetrySummary,
            source_name: "test".to_string(),
            ingest_mode: IngestMode::Snapshot,
            tenant_id: TenantId(Uuid::new_v4()),
            environment_id: None,
            observed_at: Some(topology_domain::ObservedAt(Utc::now())),
            received_at: Utc::now(),
            payload_ref: None,
            payload_inline: Some(json!({
                "collection_kind": "host_metrics",
                "metric_name": "system.target.count",
                "target_ref": "hostname:node-11:host",
                "resource_ref": "hostname:node-11",
                "value": 1
            })),
            metadata: Default::default(),
        })
        .unwrap();

        let candidate = &extracted.candidates[0];
        assert_eq!(candidate.host_name.as_deref(), Some("node-11"));
        assert_eq!(candidate.machine_id.as_deref(), Some("hostname:node-11"));
        assert_eq!(candidate.metric_name, "system.target.count");
        assert_eq!(candidate.value_i64, Some(1));
    }

    #[test]
    fn extract_process_telemetry_from_dayu_telemetry_payload() {
        let extracted = extract_process_telemetry_candidates(&IngestEnvelope {
            ingest_id: "telemetry-process-1".to_string(),
            source_kind: SourceKind::TelemetrySummary,
            source_name: "test".to_string(),
            ingest_mode: IngestMode::Snapshot,
            tenant_id: TenantId(Uuid::new_v4()),
            environment_id: None,
            observed_at: Some(topology_domain::ObservedAt(Utc::now())),
            received_at: Utc::now(),
            payload_ref: None,
            payload_inline: Some(json!({
                "collection_kind": "process_metrics",
                "metric_name": "process.memory.rss",
                "target_ref": "hostname:node-11:pid:231:started:abc:process",
                "resource_ref": "hostname:node-11:pid:231:started:abc",
                "value": 7456
            })),
            metadata: Default::default(),
        })
        .unwrap();

        let candidate = &extracted.candidates[0];
        assert_eq!(candidate.host_name.as_deref(), Some("node-11"));
        assert_eq!(candidate.machine_id.as_deref(), Some("hostname:node-11"));
        assert_eq!(candidate.pid, 231);
        assert_eq!(
            candidate.process_ref,
            "hostname:node-11:pid:231:started:abc"
        );
        assert_eq!(candidate.metric_name, "process.memory.rss");
        assert_eq!(candidate.value_i64, Some(7456));
    }

    #[test]
    fn extract_subjects_from_rows() {
        let extracted = extract_subject_candidates(&envelope(json!({
            "subjects": [{
                "display_name": "alice",
                "email": "alice@example.com",
                "subject_type": "user"
            }]
        })))
        .unwrap();

        let candidate = &extracted.candidates[0];
        assert_eq!(candidate.display_name, "alice");
        assert_eq!(candidate.email.as_deref(), Some("alice@example.com"));
    }

    #[test]
    fn extract_subjects_from_target_users_and_groups() {
        let extracted = extract_subject_candidates(&envelope(json!({
            "users": [{
                "external_id": "user-alice",
                "display_name": "Alice",
                "email": "alice@example.com"
            }],
            "groups": [{
                "external_id": "team-platform",
                "name": "platform",
                "group_type": "team"
            }]
        })))
        .unwrap();

        assert_eq!(extracted.candidates.len(), 2);
        assert_eq!(
            extracted.candidates[0].external_ref.as_deref(),
            Some("user-alice")
        );
        assert_eq!(extracted.candidates[1].display_name, "platform");
        assert!(matches!(
            extracted.candidates[1].subject_type,
            topology_domain::SubjectType::Team
        ));
    }

    #[test]
    fn extract_responsibility_assignments_from_rows() {
        let extracted = extract_responsibility_assignment_candidates(&envelope(json!({
            "responsibility_assignments": [{
                "subject_display_name": "alice",
                "subject_email": "alice@example.com",
                "target_kind": "host",
                "target_external_ref": "node-01",
                "role": "owner"
            }]
        })))
        .unwrap();

        let candidate = &extracted.candidates[0];
        assert_eq!(candidate.subject_display_name.as_deref(), Some("alice"));
        assert_eq!(
            candidate.subject_email.as_deref(),
            Some("alice@example.com")
        );
        assert!(matches!(
            candidate.target_kind,
            topology_domain::ObjectKind::Host
        ));
    }
}
