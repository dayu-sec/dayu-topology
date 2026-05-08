use chrono::{DateTime, Utc};
use orion_error::{conversion::ConvErr, prelude::SourceErr};
use serde::{Deserialize, Serialize};
use topology_domain::{
    DayuInputEnvelope, HostCandidate, IngestEnvelope, NetworkSegmentCandidate,
    ResponsibilityAssignment, Subject, SubjectCandidate,
};
use topology_storage::{
    CatalogStore, GovernanceStore, InMemoryTopologyStore, IngestJobEntry, IngestStore,
    RuntimeStore, StorageResult,
};
use uuid::Uuid;

use crate::error::{ApiReason, ApiResult, missing_payload, unsupported_ingest_mode};
use crate::ingest::{
    IngestJobRecord, IngestJobStatus, extract_host_candidates, extract_network_segment_candidates,
    extract_responsibility_assignment_candidates, extract_subject_candidates,
};
use crate::pipeline::{InMemoryCatalog, materialize_host_network};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineRunSummary {
    pub ingest_id: String,
    pub accepted_at: DateTime<Utc>,
    pub host_count: usize,
    pub network_count: usize,
    pub assoc_count: usize,
}

pub struct TopologyIngestService<S> {
    store: S,
}

impl<S> TopologyIngestService<S>
where
    S: CatalogStore + RuntimeStore + IngestStore + GovernanceStore,
{
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn submit_and_materialize(
        &self,
        envelope: IngestEnvelope,
    ) -> ApiResult<(IngestJobRecord, PipelineRunSummary)> {
        let accepted_at = envelope.received_at;
        let record = validate_and_record(&self.store, &envelope)?;

        let hosts = extract_host_candidates(&envelope)?.candidates;
        let networks = extract_network_segment_candidates(&envelope)?.candidates;
        let subjects = extract_subject_candidates(&envelope)
            .map(|result| result.candidates)
            .unwrap_or_default();
        let assignments = extract_responsibility_assignment_candidates(&envelope)
            .map(|result| result.candidates)
            .unwrap_or_default();

        let mut catalog = hydrate_catalog(&self.store, envelope.tenant_id).conv_err()?;
        let materialized =
            materialize_candidates(&self.store, &mut catalog, hosts, networks, accepted_at)
                .conv_err()?;
        materialize_subjects_and_assignments(
            &self.store,
            envelope.tenant_id,
            subjects,
            assignments,
            accepted_at,
        )
        .conv_err()?;

        Ok((
            record,
            PipelineRunSummary {
                ingest_id: envelope.ingest_id,
                accepted_at,
                host_count: materialized.0,
                network_count: materialized.1,
                assoc_count: materialized.2,
            },
        ))
    }

    pub fn submit_dayu_input_and_materialize(
        &self,
        input: DayuInputEnvelope,
        tenant_id: topology_domain::TenantId,
        environment_id: Option<topology_domain::EnvironmentId>,
    ) -> ApiResult<(IngestJobRecord, PipelineRunSummary)> {
        input.validate().conv_err()?;
        self.submit_and_materialize(input.into_ingest_envelope(
            tenant_id,
            environment_id,
            Utc::now(),
        ))
    }
}

fn validate_and_record<S>(store: &S, envelope: &IngestEnvelope) -> ApiResult<IngestJobRecord>
where
    S: IngestStore,
{
    if envelope.payload_inline.is_none() && envelope.payload_ref.is_none() {
        return Err(missing_payload());
    }

    if envelope.ingest_mode == topology_domain::IngestMode::Delta {
        return Err(unsupported_ingest_mode());
    }

    let record = IngestJobRecord {
        ingest_id: envelope.ingest_id.clone(),
        tenant_id: envelope.tenant_id,
        source_kind: envelope.source_kind,
        source_name: envelope.source_name.clone(),
        received_at: envelope.received_at,
        status: IngestJobStatus::Accepted,
        payload_ref: envelope.payload_ref.clone(),
        error: None,
    };

    store
        .record_ingest_job(IngestJobEntry {
            ingest_id: record.ingest_id.clone(),
            tenant_id: record.tenant_id,
            source_name: record.source_name.clone(),
            source_kind: format!("{:?}", record.source_kind).to_lowercase(),
            received_at: record.received_at,
            status: "accepted".to_string(),
            payload_ref: record.payload_ref.clone(),
            error: None,
        })
        .source_err(ApiReason::IngestRejected, "record ingest job")?;

    Ok(record)
}

fn hydrate_catalog<S>(
    store: &S,
    tenant_id: topology_domain::TenantId,
) -> StorageResult<InMemoryCatalog>
where
    S: CatalogStore + RuntimeStore,
{
    let hosts = store.list_hosts(tenant_id, topology_storage::Page::default())?;
    let network_segments =
        store.list_network_segments(tenant_id, topology_storage::Page::default())?;

    let mut host_net_assocs = Vec::new();
    for host in &hosts {
        host_net_assocs
            .extend(store.list_host_net_assocs(host.host_id, topology_storage::Page::default())?);
    }

    Ok(InMemoryCatalog {
        hosts,
        network_domains: Vec::new(),
        network_segments,
        host_net_assocs,
    })
}

fn materialize_candidates<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    hosts: Vec<HostCandidate>,
    networks: Vec<NetworkSegmentCandidate>,
    now: DateTime<Utc>,
) -> StorageResult<(usize, usize, usize)>
where
    S: CatalogStore + RuntimeStore,
{
    let mut host_count = 0;
    let mut network_count = 0;
    let mut assoc_count = 0;

    for network_candidate in networks {
        let host_candidate = resolve_host_candidate_for_network(&hosts, &network_candidate);
        if let Some(host_candidate) = host_candidate {
            let materialized =
                materialize_host_network(catalog, &host_candidate, &network_candidate, now);

            for domain in &catalog.network_domains {
                store.upsert_network_domain(domain)?;
            }
            store.upsert_host(&materialized.host)?;
            store.upsert_network_segment(&materialized.segment)?;
            if let Some(assoc) = &materialized.assoc {
                store.upsert_host_net_assoc(assoc)?;
                assoc_count += 1;
            }
            host_count += 1;
            network_count += 1;
        }
    }

    Ok((host_count, network_count, assoc_count))
}

fn resolve_host_candidate_for_network(
    hosts: &[HostCandidate],
    network_candidate: &NetworkSegmentCandidate,
) -> Option<HostCandidate> {
    if let Some(machine_id) = network_candidate.machine_id.as_ref() {
        if let Some(host) = hosts
            .iter()
            .find(|host| host.machine_id.as_ref() == Some(machine_id))
        {
            return Some(host.clone());
        }
    }

    if let Some(host_name) = network_candidate.host_name.as_ref() {
        if let Some(host) = hosts.iter().find(|host| &host.host_name == host_name) {
            return Some(host.clone());
        }
    }

    None
}

impl TopologyIngestService<InMemoryTopologyStore> {
    pub fn new_in_memory() -> Self {
        Self {
            store: InMemoryTopologyStore::default(),
        }
    }

    pub fn store(&self) -> &InMemoryTopologyStore {
        &self.store
    }
}

fn materialize_subjects_and_assignments<S>(
    store: &S,
    tenant_id: topology_domain::TenantId,
    subjects: Vec<SubjectCandidate>,
    assignments: Vec<topology_domain::ResponsibilityAssignmentCandidate>,
    now: DateTime<Utc>,
) -> StorageResult<()>
where
    S: CatalogStore + topology_storage::GovernanceStore,
{
    let mut persisted_subjects = Vec::new();
    for candidate in subjects {
        let subject = Subject {
            subject_id: Uuid::new_v4(),
            tenant_id,
            subject_type: candidate.subject_type,
            external_ref: candidate.external_ref,
            display_name: candidate.display_name,
            email: candidate.email,
            is_active: candidate.is_active,
            created_at: now,
            updated_at: now,
        };
        store.upsert_subject(&subject)?;
        persisted_subjects.push(subject);
    }

    let hosts = store.list_hosts(tenant_id, topology_storage::Page::default())?;
    let segments = store.list_network_segments(tenant_id, topology_storage::Page::default())?;

    for candidate in assignments {
        let subject = persisted_subjects.iter().find(|subject| {
            candidate
                .subject_email
                .as_ref()
                .is_some_and(|email| subject.email.as_ref() == Some(email))
                || candidate
                    .subject_display_name
                    .as_ref()
                    .is_some_and(|name| &subject.display_name == name)
        });

        let Some(subject) = subject else {
            continue;
        };

        let target_id = match candidate.target_kind {
            topology_domain::ObjectKind::Host => hosts.iter().find_map(|host| {
                candidate
                    .target_external_ref
                    .as_ref()
                    .filter(|target| *target == &host.host_name)
                    .map(|_| host.host_id)
            }),
            topology_domain::ObjectKind::NetworkSegment => segments.iter().find_map(|segment| {
                candidate
                    .target_external_ref
                    .as_ref()
                    .filter(|target| *target == &segment.name)
                    .map(|_| segment.network_segment_id)
            }),
            _ => None,
        };

        if let Some(target_id) = target_id {
            let assignment = ResponsibilityAssignment {
                assignment_id: Uuid::new_v4(),
                tenant_id,
                subject_id: subject.subject_id,
                target_kind: candidate.target_kind,
                target_id,
                role: candidate.role,
                source: format!("{:?}", candidate.source_kind).to_lowercase(),
                validity: candidate.validity,
                created_at: now,
                updated_at: now,
            };
            store.upsert_responsibility_assignment(&assignment)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;
    use topology_domain::{DayuInputEnvelope, IngestEnvelope, IngestMode, SourceKind, TenantId};
    use topology_storage::CatalogStore;
    use uuid::Uuid;

    use super::*;
    use crate::query::TopologyQueryService;

    #[test]
    fn submit_and_materialize_persists_minimal_host_network_closure() {
        let service = TopologyIngestService::new_in_memory();
        let tenant_id = TenantId(Uuid::new_v4());
        let envelope = IngestEnvelope {
            ingest_id: "ing-1".to_string(),
            source_kind: SourceKind::BatchImport,
            source_name: "fixture".to_string(),
            ingest_mode: IngestMode::BatchUpsert,
            tenant_id,
            environment_id: None,
            observed_at: None,
            received_at: Utc::now(),
            payload_ref: None,
            payload_inline: Some(json!({
                "hosts": [{
                    "host_name": "node-01",
                    "machine_id": "machine-01",
                    "os_name": "linux"
                }],
                "ips": [{
                    "ip": "10.0.0.12",
                    "cidr": "10.0.0.0/24",
                    "host_name": "node-01",
                    "machine_id": "machine-01",
                    "iface_name": "eth0"
                }]
            })),
            metadata: Default::default(),
        };

        let (record, summary) = service.submit_and_materialize(envelope).unwrap();

        assert_eq!(record.status, IngestJobStatus::Accepted);
        assert_eq!(summary.host_count, 1);
        assert_eq!(summary.network_count, 1);
        assert_eq!(summary.assoc_count, 1);
        assert_eq!(
            service
                .store()
                .list_hosts(tenant_id, topology_storage::Page::default())
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            service
                .store()
                .list_network_segments(tenant_id, topology_storage::Page::default())
                .unwrap()
                .len(),
            1
        );
        assert!(service.store().get_ingest_job("ing-1").unwrap().is_some());
    }

    #[test]
    fn submit_and_materialize_can_be_queried_as_topology_views() {
        let service = TopologyIngestService::new_in_memory();
        let tenant_id = TenantId(Uuid::new_v4());
        let envelope = IngestEnvelope {
            ingest_id: "ing-2".to_string(),
            source_kind: SourceKind::BatchImport,
            source_name: "fixture".to_string(),
            ingest_mode: IngestMode::BatchUpsert,
            tenant_id,
            environment_id: None,
            observed_at: None,
            received_at: Utc::now(),
            payload_ref: None,
            payload_inline: Some(json!({
                "hosts": [{
                    "host_name": "node-02",
                    "machine_id": "machine-02",
                    "os_name": "linux"
                }],
                "ips": [{
                    "ip": "10.1.0.12",
                    "cidr": "10.1.0.0/24",
                    "host_name": "node-02",
                    "machine_id": "machine-02",
                    "iface_name": "eth0"
                }]
            })),
            metadata: Default::default(),
        };

        service.submit_and_materialize(envelope).unwrap();

        let host = service
            .store()
            .list_hosts(tenant_id, topology_storage::Page::default())
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let segment = service
            .store()
            .list_network_segments(tenant_id, topology_storage::Page::default())
            .unwrap()
            .into_iter()
            .next()
            .unwrap();

        let host_view = TopologyQueryService::new(service.store().clone())
            .host_topology_view(host.host_id)
            .unwrap()
            .unwrap();
        let network_view = TopologyQueryService::new(service.store().clone())
            .network_topology_view(segment.network_segment_id)
            .unwrap()
            .unwrap();

        assert_eq!(host_view.host.host_name, "node-02");
        assert_eq!(host_view.network_segments.len(), 1);
        assert_eq!(host_view.network_assocs.len(), 1);
        assert_eq!(network_view.segment.name, "10.1.0.0/24");
        assert_eq!(network_view.hosts.len(), 1);
        assert_eq!(network_view.host_assocs.len(), 1);
    }

    #[test]
    fn submit_and_materialize_builds_effective_responsibility_view() {
        let service = TopologyIngestService::new_in_memory();
        let tenant_id = TenantId(Uuid::new_v4());
        let envelope = IngestEnvelope {
            ingest_id: "ing-3".to_string(),
            source_kind: SourceKind::BatchImport,
            source_name: "fixture".to_string(),
            ingest_mode: IngestMode::BatchUpsert,
            tenant_id,
            environment_id: None,
            observed_at: None,
            received_at: Utc::now(),
            payload_ref: None,
            payload_inline: Some(json!({
                "hosts": [{
                    "host_name": "node-03",
                    "machine_id": "machine-03",
                    "os_name": "linux"
                }],
                "ips": [{
                    "ip": "10.3.0.12",
                    "cidr": "10.3.0.0/24",
                    "host_name": "node-03",
                    "machine_id": "machine-03",
                    "iface_name": "eth0"
                }],
                "subjects": [{
                    "display_name": "alice",
                    "email": "alice@example.com",
                    "subject_type": "user"
                }],
                "responsibility_assignments": [{
                    "subject_display_name": "alice",
                    "subject_email": "alice@example.com",
                    "target_kind": "host",
                    "target_external_ref": "node-03",
                    "role": "owner"
                }]
            })),
            metadata: Default::default(),
        };

        service.submit_and_materialize(envelope).unwrap();

        let host = service
            .store()
            .list_hosts(tenant_id, topology_storage::Page::default())
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let views = TopologyQueryService::new(service.store().clone())
            .effective_responsibility_view(topology_domain::ObjectKind::Host, host.host_id)
            .unwrap();

        assert_eq!(views.len(), 1);
        assert_eq!(views[0].subject.display_name, "alice");
        assert!(matches!(
            views[0].assignment.role,
            topology_domain::ResponsibilityRole::Owner
        ));
    }

    #[test]
    fn submit_dayu_input_and_materialize_accepts_target_edge_envelope() {
        let service = TopologyIngestService::new_in_memory();
        let tenant_id = TenantId(Uuid::new_v4());
        let input: DayuInputEnvelope = serde_json::from_value(json!({
            "schema": "dayu.in.edge.v1",
            "source": {
                "system": "warp-insight",
                "producer": "agent-01",
                "tenant": "tenant-demo",
                "env": "prod"
            },
            "collect": {
                "mode": "snapshot",
                "snap_id": "snap-001",
                "observed_at": "2026-04-26T02:20:30Z"
            },
            "payload": {
                "hosts": [{
                    "hostname": "node-04",
                    "machine_id": "machine-04",
                    "os": { "name": "linux", "version": "6.8.0" }
                }],
                "interfaces": [{
                    "host_ref": "node-04",
                    "name": "eth0",
                    "addresses": [{
                        "family": "ipv4",
                        "ip": "10.4.0.12",
                        "prefix": 24,
                        "gateway": "10.4.0.1"
                    }]
                }]
            }
        }))
        .unwrap();

        let (record, summary) = service
            .submit_dayu_input_and_materialize(input, tenant_id, None)
            .unwrap();

        assert_eq!(
            record.ingest_id,
            "dayu.in.edge.v1:warp-insight:agent-01:tenant-demo:prod:snap-001"
        );
        assert_eq!(summary.host_count, 1);
        assert_eq!(summary.network_count, 1);
        assert_eq!(summary.assoc_count, 1);

        let segment = service
            .store()
            .list_network_segments(tenant_id, topology_storage::Page::default())
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(segment.name, "10.4.0.0/24");
        assert_eq!(segment.cidr.as_deref(), Some("10.4.0.0/24"));
    }
}
