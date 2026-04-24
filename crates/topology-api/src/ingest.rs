use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use topology_domain::{
    BusinessCatalogCandidate, HostCandidate, IngestEnvelope, ServiceBoundary, ServiceType,
    SourceKind,
};

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("ingest envelope is missing both payload_inline and payload_ref")]
    MissingPayload,
    #[error("payload_inline must be a JSON object")]
    PayloadMustBeObject,
    #[error("payload field `{0}` is required")]
    MissingField(&'static str),
    #[error("payload field `{0}` has invalid type")]
    InvalidFieldType(&'static str),
    #[error("payload field `{0}` has invalid value `{1}`")]
    InvalidFieldValue(&'static str, String),
    #[error("ingest job recorder failed: {0}")]
    RecorderFailed(String),
}

pub type ApiResult<T> = Result<T, ApiError>;

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
            .map_err(|err| ApiError::RecorderFailed(err.to_string()))
    }
}

impl IngestJobRecorder for InMemoryIngestJobRecorder {
    fn record_ingest_job(&self, record: IngestJobRecord) -> ApiResult<()> {
        self.records
            .lock()
            .map_err(|err| ApiError::RecorderFailed(err.to_string()))?
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
            return Err(ApiError::MissingPayload);
        }

        let record = IngestJobRecord::accepted(&envelope);
        self.recorder.record_ingest_job(record.clone())?;
        Ok(record)
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

pub fn extract_business_catalog_candidates(
    envelope: &IngestEnvelope,
) -> ApiResult<ExtractedBusinessCatalog> {
    let payload = object_payload(envelope)?;
    let rows = first_array(payload, "business_catalog", "items")?;

    let mut candidates = Vec::with_capacity(rows.len());
    for row in rows {
        let row = row
            .as_object()
            .ok_or(ApiError::InvalidFieldType("business_catalog[]"))?;
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
    let rows = first_array(payload, "hosts", "items")?;

    let mut candidates = Vec::with_capacity(rows.len());
    for row in rows {
        let row = row.as_object().ok_or(ApiError::InvalidFieldType("hosts[]"))?;
        let host_name = required_string(row, "host_name")?;

        candidates.push(HostCandidate {
            tenant_id: envelope.tenant_id,
            environment_id: envelope.environment_id,
            source_kind: envelope.source_kind,
            external_ref: optional_string(row, "external_ref")?,
            host_name,
            machine_id: optional_string(row, "machine_id")?,
            os_name: optional_string(row, "os_name")?,
            os_version: optional_string(row, "os_version")?,
        });
    }

    Ok(ExtractedHosts { candidates })
}

fn object_payload(envelope: &IngestEnvelope) -> ApiResult<&serde_json::Map<String, Value>> {
    envelope
        .payload_inline
        .as_ref()
        .ok_or(ApiError::MissingPayload)?
        .as_object()
        .ok_or(ApiError::PayloadMustBeObject)
}

fn optional_array<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<Vec<&'a Value>>> {
    match object.get(field) {
        Some(Value::Array(values)) => Ok(Some(values.iter().collect())),
        Some(_) => Err(ApiError::InvalidFieldType(field)),
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
    optional_string(object, field)?.ok_or(ApiError::MissingField(field))
}

fn optional_string(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<String>> {
    match object.get(field) {
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(Some(value.clone())),
        Some(Value::String(_)) => Ok(None),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(ApiError::InvalidFieldType(field)),
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
            _ => Err(ApiError::InvalidFieldValue(field, value)),
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
            _ => Err(ApiError::InvalidFieldValue(field, value)),
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
    use serde_json::json;
    use topology_domain::{IngestEnvelope, SourceKind, TenantId};
    use uuid::Uuid;

    use super::{
        extract_business_catalog_candidates, extract_host_candidates, InMemoryIngestJobRecorder,
        IngestJobStatus, IngestService,
    };

    fn envelope(payload_inline: serde_json::Value) -> IngestEnvelope {
        IngestEnvelope {
            ingest_id: "ing-1".to_string(),
            source_kind: SourceKind::BatchImport,
            source_name: "test".to_string(),
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
}
