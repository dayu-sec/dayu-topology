use std::collections::BTreeSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::recorder_failed;
use chrono::{DateTime, Utc};
use orion_error::{conversion::ConvErr, prelude::SourceErr};
use serde::{Deserialize, Serialize};
use topology_domain::{
    AgentHealth, BindingScope, BusinessCatalogCandidate, Confidence, DayuInputEnvelope,
    HostCandidate, HostRuntimeState, HostTelemetryCandidate, IngestEnvelope,
    NetworkSegmentCandidate, ProcessRuntimeCandidate, ProcessRuntimeState,
    ProcessTelemetryCandidate, ResponsibilityAssignment, RuntimeBinding, RuntimeObjectType,
    ServiceEntity, ServiceInstance, Subject, SubjectCandidate, ValidityWindow,
};
use topology_storage::AsyncIngestStore;
use topology_storage::{
    AsyncCatalogStore, AsyncGovernanceStore, AsyncRuntimeStore, CatalogStore, GovernanceStore,
    InMemoryTopologyStore, IngestJobEntry, IngestStore, RuntimeStore, StorageResult,
};
use uuid::Uuid;

use crate::error::{ApiReason, ApiResult, missing_payload, unsupported_ingest_mode};
use crate::ingest::{
    IngestJobRecord, IngestJobStatus, extract_business_catalog_candidates, extract_host_candidates,
    extract_host_telemetry_candidates, extract_network_segment_candidates,
    extract_process_runtime_candidates, extract_process_telemetry_candidates,
    extract_responsibility_assignment_candidates, extract_subject_candidates,
};
use crate::pipeline::{InMemoryCatalog, materialize_host_network, resolve_host_candidate};

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

impl<S> TopologyIngestService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

impl<S> TopologyIngestService<S>
where
    S: CatalogStore + RuntimeStore + IngestStore + GovernanceStore,
{
    pub fn submit_and_materialize(
        &self,
        envelope: IngestEnvelope,
    ) -> ApiResult<(IngestJobRecord, PipelineRunSummary)> {
        let accepted_at = envelope.received_at;
        let record = validate_and_record(&self.store, &envelope)?;

        let hosts = extract_host_candidates(&envelope)?.candidates;
        let networks = extract_network_segment_candidates(&envelope)?.candidates;
        let processes = extract_process_runtime_candidates(&envelope)?.candidates;
        let business_catalog = extract_business_catalog_candidates(&envelope)
            .map(|result| result.candidates)
            .unwrap_or_default();
        let host_telemetry = extract_host_telemetry_candidates(&envelope)?.candidates;
        let process_telemetry = extract_process_telemetry_candidates(&envelope)?.candidates;
        let subjects = extract_subject_candidates(&envelope)
            .map(|result| result.candidates)
            .unwrap_or_default();
        let assignments = extract_responsibility_assignment_candidates(&envelope)
            .map(|result| result.candidates)
            .unwrap_or_default();

        let mut catalog = hydrate_catalog(&self.store, envelope.tenant_id).conv_err()?;
        materialize_business_catalog(&self.store, business_catalog, accepted_at).conv_err()?;
        let materialized =
            materialize_candidates(&self.store, &mut catalog, hosts, networks, accepted_at)
                .conv_err()?;
        materialize_processes(&self.store, &mut catalog, processes, accepted_at).conv_err()?;
        materialize_host_telemetry(&self.store, &mut catalog, host_telemetry).conv_err()?;
        materialize_process_telemetry(&self.store, &mut catalog, process_telemetry).conv_err()?;
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

impl<S> TopologyIngestService<S>
where
    S: AsyncCatalogStore + AsyncRuntimeStore + AsyncIngestStore + AsyncGovernanceStore,
{
    pub async fn submit_and_materialize_async(
        &self,
        envelope: IngestEnvelope,
    ) -> ApiResult<(IngestJobRecord, PipelineRunSummary)> {
        let accepted_at = envelope.received_at;
        let record = validate_and_record_async(&self.store, &envelope).await?;

        let hosts = extract_host_candidates(&envelope)?.candidates;
        let networks = extract_network_segment_candidates(&envelope)?.candidates;
        let processes = extract_process_runtime_candidates(&envelope)?.candidates;
        let business_catalog = extract_business_catalog_candidates(&envelope)
            .map(|result| result.candidates)
            .unwrap_or_default();
        let host_telemetry = extract_host_telemetry_candidates(&envelope)?.candidates;
        let process_telemetry = extract_process_telemetry_candidates(&envelope)?.candidates;
        let subjects = extract_subject_candidates(&envelope)
            .map(|result| result.candidates)
            .unwrap_or_default();
        let assignments = extract_responsibility_assignment_candidates(&envelope)
            .map(|result| result.candidates)
            .unwrap_or_default();

        let mut catalog = hydrate_catalog_async(&self.store, envelope.tenant_id)
            .await
            .conv_err()?;
        materialize_business_catalog_async(&self.store, business_catalog, accepted_at)
            .await
            .conv_err()?;
        let materialized =
            materialize_candidates_async(&self.store, &mut catalog, hosts, networks, accepted_at)
                .await
                .conv_err()?;
        materialize_processes_async(&self.store, &mut catalog, processes, accepted_at)
            .await
            .conv_err()?;
        materialize_host_telemetry_async(&self.store, &mut catalog, host_telemetry)
            .await
            .conv_err()?;
        materialize_process_telemetry_async(&self.store, &mut catalog, process_telemetry)
            .await
            .conv_err()?;
        materialize_subjects_and_assignments_async(
            &self.store,
            envelope.tenant_id,
            subjects,
            assignments,
            accepted_at,
        )
        .await
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

    pub async fn submit_dayu_input_and_materialize_async(
        &self,
        input: DayuInputEnvelope,
        tenant_id: topology_domain::TenantId,
        environment_id: Option<topology_domain::EnvironmentId>,
    ) -> ApiResult<(IngestJobRecord, PipelineRunSummary)> {
        input.validate().conv_err()?;
        self.submit_and_materialize_async(input.into_ingest_envelope(
            tenant_id,
            environment_id,
            Utc::now(),
        ))
        .await
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

async fn validate_and_record_async<S>(
    store: &S,
    envelope: &IngestEnvelope,
) -> ApiResult<IngestJobRecord>
where
    S: AsyncIngestStore,
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

    topology_storage::AsyncIngestStore::record_ingest_job(
        store,
        IngestJobEntry {
            ingest_id: record.ingest_id.clone(),
            tenant_id: record.tenant_id,
            source_name: record.source_name.clone(),
            source_kind: format!("{:?}", record.source_kind).to_lowercase(),
            received_at: record.received_at,
            status: "accepted".to_string(),
            payload_ref: record.payload_ref.clone(),
            error: None,
        },
    )
    .await
    .map_err(|err| recorder_failed(err.to_string()))?;

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

async fn hydrate_catalog_async<S>(
    store: &S,
    tenant_id: topology_domain::TenantId,
) -> StorageResult<InMemoryCatalog>
where
    S: AsyncCatalogStore + AsyncRuntimeStore,
{
    let hosts = topology_storage::AsyncCatalogStore::list_hosts(
        store,
        tenant_id,
        topology_storage::Page::default(),
    )
    .await?;
    let network_segments = topology_storage::AsyncCatalogStore::list_network_segments(
        store,
        tenant_id,
        topology_storage::Page::default(),
    )
    .await?;

    let mut host_net_assocs = Vec::new();
    for host in &hosts {
        host_net_assocs.extend(
            topology_storage::AsyncRuntimeStore::list_host_net_assocs(
                store,
                host.host_id,
                topology_storage::Page::default(),
            )
            .await?,
        );
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
    let mut materialized_host_ids = BTreeSet::new();

    for network_candidate in networks {
        let host_candidate =
            resolve_host_candidate_for_network(&hosts, catalog, &network_candidate);
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
            materialized_host_ids.insert(materialized.host.host_id);
            host_count += 1;
            network_count += 1;
        }
    }

    for host_candidate in hosts {
        let host_resolution = resolve_host_candidate(catalog, &host_candidate, now);
        if materialized_host_ids.contains(&host_resolution.host.host_id) {
            continue;
        }

        upsert_catalog_host(catalog, host_resolution.host.clone());
        store.upsert_host(&host_resolution.host)?;
        materialized_host_ids.insert(host_resolution.host.host_id);
        host_count += 1;
    }

    Ok((host_count, network_count, assoc_count))
}

async fn materialize_candidates_async<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    hosts: Vec<HostCandidate>,
    networks: Vec<NetworkSegmentCandidate>,
    now: DateTime<Utc>,
) -> StorageResult<(usize, usize, usize)>
where
    S: AsyncCatalogStore + AsyncRuntimeStore,
{
    let mut host_count = 0;
    let mut network_count = 0;
    let mut assoc_count = 0;
    let mut materialized_host_ids = BTreeSet::new();

    for network_candidate in networks {
        let host_candidate =
            resolve_host_candidate_for_network(&hosts, catalog, &network_candidate);
        if let Some(host_candidate) = host_candidate {
            let materialized =
                materialize_host_network(catalog, &host_candidate, &network_candidate, now);

            for domain in &catalog.network_domains {
                topology_storage::AsyncCatalogStore::upsert_network_domain(store, domain).await?;
            }
            topology_storage::AsyncCatalogStore::upsert_host(store, &materialized.host).await?;
            topology_storage::AsyncCatalogStore::upsert_network_segment(
                store,
                &materialized.segment,
            )
            .await?;
            if let Some(assoc) = &materialized.assoc {
                topology_storage::AsyncRuntimeStore::upsert_host_net_assoc(store, assoc).await?;
                assoc_count += 1;
            }
            materialized_host_ids.insert(materialized.host.host_id);
            host_count += 1;
            network_count += 1;
        }
    }

    for host_candidate in hosts {
        let host_resolution = resolve_host_candidate(catalog, &host_candidate, now);
        if materialized_host_ids.contains(&host_resolution.host.host_id) {
            continue;
        }

        upsert_catalog_host(catalog, host_resolution.host.clone());
        topology_storage::AsyncCatalogStore::upsert_host(store, &host_resolution.host).await?;
        materialized_host_ids.insert(host_resolution.host.host_id);
        host_count += 1;
    }

    Ok((host_count, network_count, assoc_count))
}

fn resolve_host_candidate_for_network(
    hosts: &[HostCandidate],
    catalog: &InMemoryCatalog,
    network_candidate: &NetworkSegmentCandidate,
) -> Option<HostCandidate> {
    if let Some(machine_id) = network_candidate.machine_id.as_ref() {
        if let Some(host) = hosts
            .iter()
            .find(|host| host.machine_id.as_ref() == Some(machine_id))
        {
            return Some(host.clone());
        }

        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| host.machine_id.as_ref() == Some(machine_id))
        {
            return Some(HostCandidate {
                tenant_id: host.tenant_id,
                environment_id: host.environment_id,
                source_kind: network_candidate.source_kind,
                external_ref: None,
                host_name: host.host_name.clone(),
                machine_id: host.machine_id.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
            });
        }
    }

    if let Some(host_name) = network_candidate.host_name.as_ref() {
        if let Some(host) = hosts.iter().find(|host| &host.host_name == host_name) {
            return Some(host.clone());
        }

        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| &host.host_name == host_name)
        {
            return Some(HostCandidate {
                tenant_id: host.tenant_id,
                environment_id: host.environment_id,
                source_kind: network_candidate.source_kind,
                external_ref: None,
                host_name: host.host_name.clone(),
                machine_id: host.machine_id.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
            });
        }
    }

    None
}

fn resolve_host_candidate_for_process(
    catalog: &InMemoryCatalog,
    process_candidate: &ProcessRuntimeCandidate,
) -> Option<HostCandidate> {
    if let Some(machine_id) = process_candidate.machine_id.as_ref() {
        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| host.machine_id.as_ref() == Some(machine_id))
        {
            return Some(HostCandidate {
                tenant_id: host.tenant_id,
                environment_id: host.environment_id,
                source_kind: process_candidate.source_kind,
                external_ref: None,
                host_name: host.host_name.clone(),
                machine_id: host.machine_id.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
            });
        }
    }

    if let Some(host_name) = process_candidate.host_name.as_ref() {
        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| &host.host_name == host_name)
        {
            return Some(HostCandidate {
                tenant_id: host.tenant_id,
                environment_id: host.environment_id,
                source_kind: process_candidate.source_kind,
                external_ref: None,
                host_name: host.host_name.clone(),
                machine_id: host.machine_id.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
            });
        }
    }

    None
}

fn upsert_catalog_host(catalog: &mut InMemoryCatalog, host: topology_domain::HostInventory) {
    if let Some(existing) = catalog
        .hosts
        .iter_mut()
        .find(|existing| existing.host_id == host.host_id)
    {
        *existing = host;
    } else {
        catalog.hosts.push(host);
    }
}

fn materialize_processes<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    processes: Vec<ProcessRuntimeCandidate>,
    now: DateTime<Utc>,
) -> StorageResult<()>
where
    S: CatalogStore + RuntimeStore,
{
    for candidate in processes {
        let Some(host_candidate) = resolve_host_candidate_for_process(catalog, &candidate) else {
            continue;
        };
        let Some(host) = catalog.hosts.iter().find(|host| {
            host.host_name == host_candidate.host_name
                && host.machine_id == host_candidate.machine_id
        }) else {
            continue;
        };

        let process = ProcessRuntimeState {
            process_id: stable_uuid(
                "process_runtime",
                &candidate
                    .machine_id
                    .as_deref()
                    .zip(candidate.identity.as_deref())
                    .map(|(machine_id, identity)| {
                        format!("{machine_id}:pid:{}:{identity}", candidate.pid)
                    })
                    .unwrap_or_else(|| {
                        format!(
                            "{}:{}:{}",
                            host.host_id, candidate.pid, candidate.executable
                        )
                    }),
            ),
            tenant_id: candidate.tenant_id,
            host_id: host.host_id,
            container_id: None,
            external_ref: candidate
                .machine_id
                .as_ref()
                .zip(candidate.identity.as_ref())
                .map(|(machine_id, identity)| {
                    format!("{machine_id}:pid:{}:{identity}", candidate.pid)
                }),
            pid: candidate.pid,
            executable: candidate.executable.clone(),
            command_line: candidate.command_line.clone(),
            process_state: None,
            memory_rss_kib: None,
            started_at: candidate.observed_at.map(|item| item.0).unwrap_or(now),
            observed_at: candidate
                .observed_at
                .unwrap_or(topology_domain::ObservedAt(now)),
        };
        store.upsert_process_runtime_state(&process)?;
        materialize_process_binding(store, &candidate, &process, now)?;
    }

    Ok(())
}

async fn materialize_processes_async<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    processes: Vec<ProcessRuntimeCandidate>,
    now: DateTime<Utc>,
) -> StorageResult<()>
where
    S: AsyncCatalogStore + AsyncRuntimeStore,
{
    for candidate in processes {
        let Some(host_candidate) = resolve_host_candidate_for_process(catalog, &candidate) else {
            continue;
        };
        let Some(host) = catalog.hosts.iter().find(|host| {
            host.host_name == host_candidate.host_name
                && host.machine_id == host_candidate.machine_id
        }) else {
            continue;
        };

        let process = ProcessRuntimeState {
            process_id: stable_uuid(
                "process_runtime",
                &candidate
                    .machine_id
                    .as_deref()
                    .zip(candidate.identity.as_deref())
                    .map(|(machine_id, identity)| {
                        format!("{machine_id}:pid:{}:{identity}", candidate.pid)
                    })
                    .unwrap_or_else(|| {
                        format!(
                            "{}:{}:{}",
                            host.host_id, candidate.pid, candidate.executable
                        )
                    }),
            ),
            tenant_id: candidate.tenant_id,
            host_id: host.host_id,
            container_id: None,
            external_ref: candidate
                .machine_id
                .as_ref()
                .zip(candidate.identity.as_ref())
                .map(|(machine_id, identity)| {
                    format!("{machine_id}:pid:{}:{identity}", candidate.pid)
                }),
            pid: candidate.pid,
            executable: candidate.executable.clone(),
            command_line: candidate.command_line.clone(),
            process_state: None,
            memory_rss_kib: None,
            started_at: candidate.observed_at.map(|item| item.0).unwrap_or(now),
            observed_at: candidate
                .observed_at
                .unwrap_or(topology_domain::ObservedAt(now)),
        };
        topology_storage::AsyncRuntimeStore::upsert_process_runtime_state(store, &process).await?;
        materialize_process_binding_async(store, &candidate, &process, now).await?;
    }

    Ok(())
}

fn materialize_process_telemetry<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    metrics: Vec<ProcessTelemetryCandidate>,
) -> StorageResult<()>
where
    S: CatalogStore + RuntimeStore,
{
    let full_page = topology_storage::Page {
        limit: i32::MAX as u32,
        offset: 0,
    };
    for candidate in metrics {
        let Some(host_candidate) =
            resolve_host_candidate_for_process_telemetry(catalog, &candidate)
        else {
            continue;
        };
        let Some(host) = catalog.hosts.iter().find(|host| {
            host.host_name == host_candidate.host_name
                && host.machine_id == host_candidate.machine_id
        }) else {
            continue;
        };

        let Some(mut process) = store
            .list_process_runtime_states(host.host_id, full_page)?
            .into_iter()
            .find(|state| {
                state.external_ref.as_deref() == Some(candidate.process_ref.as_str())
                    || (state.pid == candidate.pid
                        && state.external_ref.as_deref().is_some_and(|value| {
                            value.starts_with(&format!(
                                "{}:pid:{}:",
                                candidate.machine_id.as_deref().unwrap_or(""),
                                candidate.pid
                            ))
                        }))
            })
        else {
            continue;
        };

        match candidate.metric_name.as_str() {
            "process.state" => process.process_state = candidate.value_string.clone(),
            "process.memory.rss" => process.memory_rss_kib = candidate.value_i64,
            _ => continue,
        }
        process.observed_at = candidate.observed_at;

        store.upsert_process_runtime_state(&process)?;
    }

    Ok(())
}

async fn materialize_process_telemetry_async<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    metrics: Vec<ProcessTelemetryCandidate>,
) -> StorageResult<()>
where
    S: AsyncCatalogStore + AsyncRuntimeStore,
{
    let full_page = topology_storage::Page {
        limit: i32::MAX as u32,
        offset: 0,
    };
    for candidate in metrics {
        let Some(host_candidate) =
            resolve_host_candidate_for_process_telemetry(catalog, &candidate)
        else {
            continue;
        };
        let Some(host) = catalog.hosts.iter().find(|host| {
            host.host_name == host_candidate.host_name
                && host.machine_id == host_candidate.machine_id
        }) else {
            continue;
        };

        let Some(mut process) = topology_storage::AsyncRuntimeStore::list_process_runtime_states(
            store,
            host.host_id,
            full_page,
        )
        .await?
        .into_iter()
        .find(|state| {
            state.external_ref.as_deref() == Some(candidate.process_ref.as_str())
                || (state.pid == candidate.pid
                    && state.external_ref.as_deref().is_some_and(|value| {
                        value.starts_with(&format!(
                            "{}:pid:{}:",
                            candidate.machine_id.as_deref().unwrap_or(""),
                            candidate.pid
                        ))
                    }))
        }) else {
            continue;
        };

        match candidate.metric_name.as_str() {
            "process.state" => process.process_state = candidate.value_string.clone(),
            "process.memory.rss" => process.memory_rss_kib = candidate.value_i64,
            _ => continue,
        }
        process.observed_at = candidate.observed_at;

        topology_storage::AsyncRuntimeStore::upsert_process_runtime_state(store, &process).await?;
    }

    Ok(())
}

fn materialize_host_telemetry<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    metrics: Vec<HostTelemetryCandidate>,
) -> StorageResult<()>
where
    S: CatalogStore + RuntimeStore,
{
    for candidate in metrics {
        let Some(host_candidate) = resolve_host_candidate_for_host_telemetry(catalog, &candidate)
        else {
            continue;
        };
        let Some(host) = catalog.hosts.iter().find(|host| {
            host.host_name == host_candidate.host_name
                && host.machine_id == host_candidate.machine_id
        }) else {
            continue;
        };

        let mut runtime = store
            .list_host_runtime_states(host.host_id, topology_storage::Page::default())?
            .into_iter()
            .find(|state| state.observed_at == candidate.observed_at)
            .unwrap_or(HostRuntimeState {
                host_id: host.host_id,
                observed_at: candidate.observed_at,
                boot_id: None,
                uptime_seconds: None,
                loadavg_1m: None,
                loadavg_5m: None,
                loadavg_15m: None,
                cpu_usage_pct: None,
                memory_used_bytes: None,
                memory_available_bytes: None,
                process_count: None,
                container_count: None,
                agent_health: AgentHealth::Healthy,
            });

        match candidate.metric_name.as_str() {
            "system.target.count" => runtime.process_count = candidate.value_i64,
            "system.load_average.1m" => runtime.loadavg_1m = candidate.value_f64,
            "system.load_average.5m" => runtime.loadavg_5m = candidate.value_f64,
            "system.load_average.15m" => runtime.loadavg_15m = candidate.value_f64,
            "system.memory.used" | "system.memory.used_bytes" => {
                runtime.memory_used_bytes = candidate.value_i64
            }
            "system.memory.available" | "system.memory.available_bytes" => {
                runtime.memory_available_bytes = candidate.value_i64
            }
            "system.container.count" => runtime.container_count = candidate.value_i64,
            _ => continue,
        }

        store.insert_host_runtime_state(&runtime)?;
    }

    Ok(())
}

async fn materialize_host_telemetry_async<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    metrics: Vec<HostTelemetryCandidate>,
) -> StorageResult<()>
where
    S: AsyncCatalogStore + AsyncRuntimeStore,
{
    for candidate in metrics {
        let Some(host_candidate) = resolve_host_candidate_for_host_telemetry(catalog, &candidate)
        else {
            continue;
        };
        let Some(host) = catalog.hosts.iter().find(|host| {
            host.host_name == host_candidate.host_name
                && host.machine_id == host_candidate.machine_id
        }) else {
            continue;
        };

        let mut runtime = topology_storage::AsyncRuntimeStore::list_host_runtime_states(
            store,
            host.host_id,
            topology_storage::Page::default(),
        )
        .await?
        .into_iter()
        .find(|state| state.observed_at == candidate.observed_at)
        .unwrap_or(HostRuntimeState {
            host_id: host.host_id,
            observed_at: candidate.observed_at,
            boot_id: None,
            uptime_seconds: None,
            loadavg_1m: None,
            loadavg_5m: None,
            loadavg_15m: None,
            cpu_usage_pct: None,
            memory_used_bytes: None,
            memory_available_bytes: None,
            process_count: None,
            container_count: None,
            agent_health: AgentHealth::Healthy,
        });

        match candidate.metric_name.as_str() {
            "system.target.count" => runtime.process_count = candidate.value_i64,
            "system.load_average.1m" => runtime.loadavg_1m = candidate.value_f64,
            "system.load_average.5m" => runtime.loadavg_5m = candidate.value_f64,
            "system.load_average.15m" => runtime.loadavg_15m = candidate.value_f64,
            "system.memory.used" | "system.memory.used_bytes" => {
                runtime.memory_used_bytes = candidate.value_i64
            }
            "system.memory.available" | "system.memory.available_bytes" => {
                runtime.memory_available_bytes = candidate.value_i64
            }
            "system.container.count" => runtime.container_count = candidate.value_i64,
            _ => continue,
        }

        topology_storage::AsyncRuntimeStore::insert_host_runtime_state(store, &runtime).await?;
    }

    Ok(())
}

fn resolve_host_candidate_for_host_telemetry(
    catalog: &InMemoryCatalog,
    candidate: &HostTelemetryCandidate,
) -> Option<HostCandidate> {
    if let Some(machine_id) = candidate.machine_id.as_ref() {
        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| host.machine_id.as_ref() == Some(machine_id))
        {
            return Some(HostCandidate {
                tenant_id: host.tenant_id,
                environment_id: host.environment_id,
                source_kind: candidate.source_kind,
                external_ref: None,
                host_name: host.host_name.clone(),
                machine_id: host.machine_id.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
            });
        }
    }

    if let Some(host_name) = candidate.host_name.as_ref() {
        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| &host.host_name == host_name)
        {
            return Some(HostCandidate {
                tenant_id: host.tenant_id,
                environment_id: host.environment_id,
                source_kind: candidate.source_kind,
                external_ref: None,
                host_name: host.host_name.clone(),
                machine_id: host.machine_id.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
            });
        }
    }

    None
}

fn resolve_host_candidate_for_process_telemetry(
    catalog: &InMemoryCatalog,
    candidate: &ProcessTelemetryCandidate,
) -> Option<HostCandidate> {
    if let Some(machine_id) = candidate.machine_id.as_ref() {
        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| host.machine_id.as_ref() == Some(machine_id))
        {
            return Some(HostCandidate {
                tenant_id: host.tenant_id,
                environment_id: host.environment_id,
                source_kind: candidate.source_kind,
                external_ref: None,
                host_name: host.host_name.clone(),
                machine_id: host.machine_id.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
            });
        }
    }

    if let Some(host_name) = candidate.host_name.as_ref() {
        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| &host.host_name == host_name)
        {
            return Some(HostCandidate {
                tenant_id: host.tenant_id,
                environment_id: host.environment_id,
                source_kind: candidate.source_kind,
                external_ref: None,
                host_name: host.host_name.clone(),
                machine_id: host.machine_id.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
            });
        }
    }

    None
}

fn materialize_process_binding<S>(
    store: &S,
    candidate: &ProcessRuntimeCandidate,
    process: &ProcessRuntimeState,
    now: DateTime<Utc>,
) -> StorageResult<()>
where
    S: CatalogStore + RuntimeStore,
{
    let Some(service_ref) = candidate.service_ref.as_deref() else {
        return Ok(());
    };

    let service = find_service_by_ref(store, candidate.tenant_id, service_ref)?;
    let Some(service) = service else {
        return Ok(());
    };

    let instance_key = candidate
        .instance_key
        .as_deref()
        .or(candidate.identity.as_deref())
        .unwrap_or("process");
    let instance_id = stable_uuid(
        "service_instance",
        &format!(
            "{}:{}:{}",
            candidate.tenant_id.0, service.service_id, instance_key
        ),
    );
    let instance_started_at = process.started_at;
    let binding_valid_from = process.observed_at.0;
    let instance = ServiceInstance {
        instance_id,
        tenant_id: candidate.tenant_id,
        service_id: service.service_id,
        workload_id: None,
        started_at: instance_started_at,
        ended_at: None,
        last_seen_at: process.observed_at.0,
    };
    store.upsert_service_instance(&instance)?;

    let binding_id = stable_uuid(
        "runtime_binding",
        &format!(
            "{}:{}:{}",
            instance.instance_id, process.process_id, "process"
        ),
    );
    let binding = RuntimeBinding {
        binding_id,
        instance_id: instance.instance_id,
        object_type: RuntimeObjectType::Process,
        object_id: process.process_id,
        scope: BindingScope::Observed,
        confidence: Confidence::Medium,
        source: format!("{:?}", candidate.source_kind).to_lowercase(),
        validity: ValidityWindow {
            valid_from: binding_valid_from,
            valid_to: None,
        },
        created_at: now,
        updated_at: now,
    };
    store.upsert_runtime_binding(&binding)?;

    Ok(())
}

fn materialize_business_catalog<S>(
    store: &S,
    candidates: Vec<BusinessCatalogCandidate>,
    now: DateTime<Utc>,
) -> StorageResult<()>
where
    S: CatalogStore,
{
    for candidate in candidates {
        let Some(service_name) = candidate.service_name.as_deref() else {
            continue;
        };
        let service_ref = candidate
            .external_ref
            .clone()
            .unwrap_or_else(|| service_name.to_string());
        let service = ServiceEntity {
            service_id: stable_uuid(
                "service",
                &format!("{}:{}", candidate.tenant_id.0, service_ref),
            ),
            tenant_id: candidate.tenant_id,
            business_id: None,
            system_id: None,
            subsystem_id: None,
            name: service_name.to_string(),
            namespace: None,
            service_type: candidate
                .service_type
                .unwrap_or(topology_domain::ServiceType::Application),
            boundary: candidate
                .boundary
                .unwrap_or(topology_domain::ServiceBoundary::Internal),
            provider: None,
            external_ref: candidate.external_ref.clone(),
            created_at: now,
            updated_at: now,
        };
        store.upsert_service(&service)?;
    }
    Ok(())
}

async fn materialize_business_catalog_async<S>(
    store: &S,
    candidates: Vec<BusinessCatalogCandidate>,
    now: DateTime<Utc>,
) -> StorageResult<()>
where
    S: AsyncCatalogStore,
{
    for candidate in candidates {
        let Some(service_name) = candidate.service_name.as_deref() else {
            continue;
        };
        let service_ref = candidate
            .external_ref
            .clone()
            .unwrap_or_else(|| service_name.to_string());
        let service = ServiceEntity {
            service_id: stable_uuid(
                "service",
                &format!("{}:{}", candidate.tenant_id.0, service_ref),
            ),
            tenant_id: candidate.tenant_id,
            business_id: None,
            system_id: None,
            subsystem_id: None,
            name: service_name.to_string(),
            namespace: None,
            service_type: candidate
                .service_type
                .unwrap_or(topology_domain::ServiceType::Application),
            boundary: candidate
                .boundary
                .unwrap_or(topology_domain::ServiceBoundary::Internal),
            provider: None,
            external_ref: candidate.external_ref.clone(),
            created_at: now,
            updated_at: now,
        };
        topology_storage::AsyncCatalogStore::upsert_service(store, &service).await?;
    }
    Ok(())
}

async fn materialize_process_binding_async<S>(
    store: &S,
    candidate: &ProcessRuntimeCandidate,
    process: &ProcessRuntimeState,
    now: DateTime<Utc>,
) -> StorageResult<()>
where
    S: AsyncCatalogStore + AsyncRuntimeStore,
{
    let Some(service_ref) = candidate.service_ref.as_deref() else {
        return Ok(());
    };

    let service = find_service_by_ref_async(store, candidate.tenant_id, service_ref).await?;
    let Some(service) = service else {
        return Ok(());
    };

    let instance_key = candidate
        .instance_key
        .as_deref()
        .or(candidate.identity.as_deref())
        .unwrap_or("process");
    let instance_id = stable_uuid(
        "service_instance",
        &format!(
            "{}:{}:{}",
            candidate.tenant_id.0, service.service_id, instance_key
        ),
    );
    let instance_started_at = process.started_at;
    let binding_valid_from = process.observed_at.0;
    let instance = ServiceInstance {
        instance_id,
        tenant_id: candidate.tenant_id,
        service_id: service.service_id,
        workload_id: None,
        started_at: instance_started_at,
        ended_at: None,
        last_seen_at: process.observed_at.0,
    };
    topology_storage::AsyncRuntimeStore::upsert_service_instance(store, &instance).await?;

    let binding_id = stable_uuid(
        "runtime_binding",
        &format!(
            "{}:{}:{}",
            instance.instance_id, process.process_id, "process"
        ),
    );
    let binding = RuntimeBinding {
        binding_id,
        instance_id: instance.instance_id,
        object_type: RuntimeObjectType::Process,
        object_id: process.process_id,
        scope: BindingScope::Observed,
        confidence: Confidence::Medium,
        source: format!("{:?}", candidate.source_kind).to_lowercase(),
        validity: ValidityWindow {
            valid_from: binding_valid_from,
            valid_to: None,
        },
        created_at: now,
        updated_at: now,
    };
    topology_storage::AsyncRuntimeStore::upsert_runtime_binding(store, &binding).await?;

    Ok(())
}

fn stable_uuid(namespace: &str, key: &str) -> Uuid {
    let mut h1 = DefaultHasher::new();
    ("dayu-topology", namespace, key, "a").hash(&mut h1);
    let mut h2 = DefaultHasher::new();
    ("dayu-topology", namespace, key, "b").hash(&mut h2);
    let hi = h1.finish().to_be_bytes();
    let lo = h2.finish().to_be_bytes();
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&hi);
    bytes[8..].copy_from_slice(&lo);
    Uuid::from_bytes(bytes)
}

fn find_service_by_ref<S>(
    store: &S,
    tenant_id: topology_domain::TenantId,
    service_ref: &str,
) -> StorageResult<Option<ServiceEntity>>
where
    S: CatalogStore,
{
    let services = store.list_services(tenant_id, topology_storage::Page::default())?;
    Ok(services.into_iter().find(|service| {
        service.external_ref.as_deref() == Some(service_ref) || service.name == service_ref
    }))
}

async fn find_service_by_ref_async<S>(
    store: &S,
    tenant_id: topology_domain::TenantId,
    service_ref: &str,
) -> StorageResult<Option<ServiceEntity>>
where
    S: AsyncCatalogStore,
{
    let services = topology_storage::AsyncCatalogStore::list_services(
        store,
        tenant_id,
        topology_storage::Page::default(),
    )
    .await?;
    Ok(services.into_iter().find(|service| {
        service.external_ref.as_deref() == Some(service_ref) || service.name == service_ref
    }))
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

async fn materialize_subjects_and_assignments_async<S>(
    store: &S,
    tenant_id: topology_domain::TenantId,
    subjects: Vec<SubjectCandidate>,
    assignments: Vec<topology_domain::ResponsibilityAssignmentCandidate>,
    now: DateTime<Utc>,
) -> StorageResult<()>
where
    S: AsyncCatalogStore + AsyncGovernanceStore,
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
        topology_storage::AsyncCatalogStore::upsert_subject(store, &subject).await?;
        persisted_subjects.push(subject);
    }

    let hosts = topology_storage::AsyncCatalogStore::list_hosts(
        store,
        tenant_id,
        topology_storage::Page::default(),
    )
    .await?;
    let segments = topology_storage::AsyncCatalogStore::list_network_segments(
        store,
        tenant_id,
        topology_storage::Page::default(),
    )
    .await?;

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
            topology_storage::AsyncGovernanceStore::upsert_responsibility_assignment(
                store,
                &assignment,
            )
            .await?;
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
            CatalogStore::list_hosts(
                service.store(),
                tenant_id,
                topology_storage::Page::default(),
            )
            .unwrap()
            .len(),
            1
        );
        assert_eq!(
            CatalogStore::list_network_segments(
                service.store(),
                tenant_id,
                topology_storage::Page::default(),
            )
            .unwrap()
            .len(),
            1
        );
        assert!(
            IngestStore::get_ingest_job(service.store(), "ing-1")
                .unwrap()
                .is_some()
        );
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

        let host = CatalogStore::list_hosts(
            service.store(),
            tenant_id,
            topology_storage::Page::default(),
        )
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
        let segment = CatalogStore::list_network_segments(
            service.store(),
            tenant_id,
            topology_storage::Page::default(),
        )
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

        let host = CatalogStore::list_hosts(
            service.store(),
            tenant_id,
            topology_storage::Page::default(),
        )
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

        let segment = CatalogStore::list_network_segments(
            service.store(),
            tenant_id,
            topology_storage::Page::default(),
        )
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
        assert_eq!(segment.name, "10.4.0.0/24");
        assert_eq!(segment.cidr.as_deref(), Some("10.4.0.0/24"));
    }

    #[test]
    fn submit_and_materialize_persists_host_without_network_candidates() {
        let service = TopologyIngestService::new_in_memory();
        let tenant_id = TenantId(Uuid::new_v4());
        let envelope = IngestEnvelope {
            ingest_id: "ing-host-only".to_string(),
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
                    "host_name": "node-host-only",
                    "machine_id": "machine-host-only",
                    "external_ref": "machine-host-only"
                }]
            })),
            metadata: Default::default(),
        };

        let (record, summary) = service.submit_and_materialize(envelope).unwrap();

        assert_eq!(record.status, IngestJobStatus::Accepted);
        assert_eq!(summary.host_count, 1);
        assert_eq!(summary.network_count, 0);
        assert_eq!(summary.assoc_count, 0);

        let hosts = CatalogStore::list_hosts(
            service.store(),
            tenant_id,
            topology_storage::Page::default(),
        )
        .unwrap();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].host_name, "node-host-only");
        assert_eq!(hosts[0].machine_id.as_deref(), Some("machine-host-only"));
    }

    #[test]
    fn submit_and_materialize_links_network_fact_to_previously_materialized_host() {
        let service = TopologyIngestService::new_in_memory();
        let tenant_id = TenantId(Uuid::new_v4());

        let host_envelope = IngestEnvelope {
            ingest_id: "ing-host-seed".to_string(),
            source_kind: SourceKind::EdgeDiscovery,
            source_name: "warp-insight:agent-01".to_string(),
            ingest_mode: IngestMode::Snapshot,
            tenant_id,
            environment_id: None,
            observed_at: None,
            received_at: Utc::now(),
            payload_ref: None,
            payload_inline: Some(json!({
                "target_kind": "host",
                "host_name": "node-07",
                "machine_id": "hostname:node-07",
                "external_ref": "hostname:node-07"
            })),
            metadata: Default::default(),
        };
        service.submit_and_materialize(host_envelope).unwrap();

        let network_envelope = IngestEnvelope {
            ingest_id: "ing-network-followup".to_string(),
            source_kind: SourceKind::EdgeDiscovery,
            source_name: "warp-insight:agent-01".to_string(),
            ingest_mode: IngestMode::Snapshot,
            tenant_id,
            environment_id: None,
            observed_at: None,
            received_at: Utc::now(),
            payload_ref: None,
            payload_inline: Some(json!({
                "target_kind": "network_interface",
                "host_name": "node-07",
                "machine_id": "hostname:node-07",
                "iface_name": "eth0",
                "ip": "10.7.0.12",
                "prefix": 24,
                "gateway": "10.7.0.1"
            })),
            metadata: Default::default(),
        };

        let (_record, summary) = service.submit_and_materialize(network_envelope).unwrap();

        assert_eq!(summary.host_count, 1);
        assert_eq!(summary.network_count, 1);
        assert_eq!(summary.assoc_count, 1);

        let host = CatalogStore::list_hosts(
            service.store(),
            tenant_id,
            topology_storage::Page::default(),
        )
        .unwrap()
        .into_iter()
        .find(|host| host.host_name == "node-07")
        .unwrap();
        let host_view = TopologyQueryService::new(service.store().clone())
            .host_topology_view(host.host_id)
            .unwrap()
            .unwrap();

        assert_eq!(host_view.network_segments.len(), 1);
        assert_eq!(
            host_view.network_segments[0].cidr.as_deref(),
            Some("10.7.0.0/24")
        );
        assert_eq!(host_view.network_assocs.len(), 1);
        assert_eq!(host_view.network_assocs[0].ip_addr, "10.7.0.12");
    }

    #[test]
    fn submit_and_materialize_persists_process_fact_for_existing_host() {
        let service = TopologyIngestService::new_in_memory();
        let tenant_id = TenantId(Uuid::new_v4());

        let host_envelope = IngestEnvelope {
            ingest_id: "ing-host-for-process".to_string(),
            source_kind: SourceKind::EdgeDiscovery,
            source_name: "warp-insight:agent-01".to_string(),
            ingest_mode: IngestMode::Snapshot,
            tenant_id,
            environment_id: None,
            observed_at: None,
            received_at: Utc::now(),
            payload_ref: None,
            payload_inline: Some(json!({
                "target_kind": "host",
                "host_name": "node-09",
                "machine_id": "hostname:node-09",
                "external_ref": "hostname:node-09"
            })),
            metadata: Default::default(),
        };
        service.submit_and_materialize(host_envelope).unwrap();

        let process_envelope = IngestEnvelope {
            ingest_id: "ing-process-followup".to_string(),
            source_kind: SourceKind::EdgeDiscovery,
            source_name: "warp-insight:agent-01".to_string(),
            ingest_mode: IngestMode::Snapshot,
            tenant_id,
            environment_id: None,
            observed_at: None,
            received_at: Utc::now(),
            payload_ref: None,
            payload_inline: Some(json!({
                "target_kind": "process",
                "host_name": "node-09",
                "machine_id": "hostname:node-09",
                "pid": "231",
                "identity": "ps_lstart:Tue May 12 05:38:01 2026",
                "process_key": "hostname:node-09:pid:231:ps_lstart:Tue May 12 05:38:01 2026",
                "executable_name": "/usr/sbin/sshd",
                "observed_at": "2026-05-12T03:16:03Z"
            })),
            metadata: Default::default(),
        };

        service.submit_and_materialize(process_envelope).unwrap();

        let host = CatalogStore::list_hosts(
            service.store(),
            tenant_id,
            topology_storage::Page::default(),
        )
        .unwrap()
        .into_iter()
        .find(|host| host.host_name == "node-09")
        .unwrap();
        let processes = RuntimeStore::list_process_runtime_states(
            service.store(),
            host.host_id,
            topology_storage::Page::default(),
        )
        .unwrap();

        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].pid, 231);
        assert_eq!(processes[0].executable, "/usr/sbin/sshd");
    }

    #[test]
    fn submit_and_materialize_links_process_fact_using_process_key_host_prefix() {
        let service = TopologyIngestService::new_in_memory();
        let tenant_id = TenantId(Uuid::new_v4());

        let host_envelope = IngestEnvelope {
            ingest_id: "ing-host-for-derived-process".to_string(),
            source_kind: SourceKind::EdgeDiscovery,
            source_name: "warp-insight:agent-01".to_string(),
            ingest_mode: IngestMode::Snapshot,
            tenant_id,
            environment_id: None,
            observed_at: None,
            received_at: Utc::now(),
            payload_ref: None,
            payload_inline: Some(json!({
                "target_kind": "host",
                "host_name": "node-09",
                "machine_id": "hostname:node-09",
                "external_ref": "hostname:node-09"
            })),
            metadata: Default::default(),
        };
        service.submit_and_materialize(host_envelope).unwrap();

        let process_envelope = IngestEnvelope {
            ingest_id: "ing-process-followup-derived".to_string(),
            source_kind: SourceKind::EdgeDiscovery,
            source_name: "warp-insight:agent-01".to_string(),
            ingest_mode: IngestMode::Snapshot,
            tenant_id,
            environment_id: None,
            observed_at: None,
            received_at: Utc::now(),
            payload_ref: None,
            payload_inline: Some(json!({
                "target_kind": "process",
                "pid": "231",
                "identity": "ps_lstart:Tue May 12 05:38:01 2026",
                "process_key": "hostname:node-09:pid:231:ps_lstart:Tue May 12 05:38:01 2026",
                "executable_name": "/usr/sbin/sshd",
                "observed_at": "2026-05-12T03:16:03Z"
            })),
            metadata: Default::default(),
        };

        service.submit_and_materialize(process_envelope).unwrap();

        let host = CatalogStore::list_hosts(
            service.store(),
            tenant_id,
            topology_storage::Page::default(),
        )
        .unwrap()
        .into_iter()
        .find(|host| host.host_name == "node-09")
        .unwrap();
        let processes = RuntimeStore::list_process_runtime_states(
            service.store(),
            host.host_id,
            topology_storage::Page::default(),
        )
        .unwrap();

        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].pid, 231);
        assert_eq!(processes[0].executable, "/usr/sbin/sshd");
    }

    #[test]
    fn submit_and_materialize_enriches_existing_process_from_telemetry() {
        let service = TopologyIngestService::new_in_memory();
        let tenant_id = TenantId(Uuid::new_v4());

        let host_envelope = IngestEnvelope {
            ingest_id: "ing-host-for-process-telemetry".to_string(),
            source_kind: SourceKind::EdgeDiscovery,
            source_name: "warp-insight:agent-01".to_string(),
            ingest_mode: IngestMode::Snapshot,
            tenant_id,
            environment_id: None,
            observed_at: None,
            received_at: Utc::now(),
            payload_ref: None,
            payload_inline: Some(json!({
                "target_kind": "host",
                "host_name": "node-11",
                "machine_id": "hostname:node-11",
                "external_ref": "hostname:node-11"
            })),
            metadata: Default::default(),
        };
        service.submit_and_materialize(host_envelope).unwrap();

        let process_envelope = IngestEnvelope {
            ingest_id: "ing-process-for-telemetry".to_string(),
            source_kind: SourceKind::EdgeDiscovery,
            source_name: "warp-insight:agent-01".to_string(),
            ingest_mode: IngestMode::Snapshot,
            tenant_id,
            environment_id: None,
            observed_at: None,
            received_at: Utc::now(),
            payload_ref: None,
            payload_inline: Some(json!({
                "target_kind": "process",
                "pid": "231",
                "identity": "ps_lstart:Tue May 12 05:38:01 2026",
                "process_key": "hostname:node-11:pid:231:ps_lstart:Tue May 12 05:38:01 2026",
                "executable_name": "/usr/sbin/sshd",
                "observed_at": "2026-05-12T03:16:03Z"
            })),
            metadata: Default::default(),
        };
        service.submit_and_materialize(process_envelope).unwrap();

        let telemetry_envelope = IngestEnvelope {
            ingest_id: "ing-process-telemetry".to_string(),
            source_kind: SourceKind::TelemetrySummary,
            source_name: "warp-insight:agent-01".to_string(),
            ingest_mode: IngestMode::Snapshot,
            tenant_id,
            environment_id: None,
            observed_at: Some(topology_domain::ObservedAt(
                chrono::DateTime::parse_from_rfc3339("2026-05-12T03:16:04Z")
                    .unwrap()
                    .with_timezone(&Utc),
            )),
            received_at: Utc::now(),
            payload_ref: None,
            payload_inline: Some(json!({
                "collection_kind": "process_metrics",
                "metric_name": "process.memory.rss",
                "resource_ref": "hostname:node-11:pid:231:ps_lstart:Tue May 12 05:38:01 2026",
                "target_ref": "hostname:node-11:pid:231:ps_lstart:Tue May 12 05:38:01 2026:process",
                "value": 7456
            })),
            metadata: Default::default(),
        };
        service.submit_and_materialize(telemetry_envelope).unwrap();

        let host = CatalogStore::list_hosts(
            service.store(),
            tenant_id,
            topology_storage::Page::default(),
        )
        .unwrap()
        .into_iter()
        .find(|host| host.host_name == "node-11")
        .unwrap();
        let processes = RuntimeStore::list_process_runtime_states(
            service.store(),
            host.host_id,
            topology_storage::Page::default(),
        )
        .unwrap();

        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].memory_rss_kib, Some(7456));
        assert_eq!(
            processes[0].external_ref.as_deref(),
            Some("hostname:node-11:pid:231:ps_lstart:Tue May 12 05:38:01 2026")
        );
    }

    #[test]
    fn submit_and_materialize_creates_runtime_binding_when_service_ref_is_present() {
        let service = TopologyIngestService::new_in_memory();
        let tenant_id = TenantId(Uuid::new_v4());
        let now = Utc::now();

        CatalogStore::upsert_service(
            service.store(),
            &topology_domain::ServiceEntity {
                service_id: Uuid::new_v4(),
                tenant_id,
                business_id: None,
                system_id: None,
                subsystem_id: None,
                name: "sshd".to_string(),
                namespace: None,
                service_type: topology_domain::ServiceType::Platform,
                boundary: topology_domain::ServiceBoundary::Internal,
                provider: None,
                external_ref: Some("svc:sshd".to_string()),
                created_at: now,
                updated_at: now,
            },
        )
        .unwrap();

        let host_envelope = IngestEnvelope {
            ingest_id: "ing-host-for-binding".to_string(),
            source_kind: SourceKind::EdgeDiscovery,
            source_name: "warp-insight:agent-01".to_string(),
            ingest_mode: IngestMode::Snapshot,
            tenant_id,
            environment_id: None,
            observed_at: None,
            received_at: now,
            payload_ref: None,
            payload_inline: Some(json!({
                "target_kind": "host",
                "host_name": "node-10",
                "machine_id": "hostname:node-10",
                "external_ref": "hostname:node-10"
            })),
            metadata: Default::default(),
        };
        service.submit_and_materialize(host_envelope).unwrap();

        let process_envelope = IngestEnvelope {
            ingest_id: "ing-process-binding".to_string(),
            source_kind: SourceKind::EdgeDiscovery,
            source_name: "warp-insight:agent-01".to_string(),
            ingest_mode: IngestMode::Snapshot,
            tenant_id,
            environment_id: None,
            observed_at: None,
            received_at: now,
            payload_ref: None,
            payload_inline: Some(json!({
                "target_kind": "process",
                "host_name": "node-10",
                "machine_id": "hostname:node-10",
                "pid": "222",
                "identity": "sshd:instance-a",
                "process_key": "hostname:node-10:pid:222:sshd:instance-a",
                "executable_name": "/usr/sbin/sshd",
                "service_ref": "svc:sshd",
                "instance_key": "process:sshd:instance-a",
                "observed_at": "2026-05-12T03:16:03Z"
            })),
            metadata: Default::default(),
        };
        service.submit_and_materialize(process_envelope).unwrap();

        let service_entity = CatalogStore::list_services(
            service.store(),
            tenant_id,
            topology_storage::Page::default(),
        )
        .unwrap()
        .into_iter()
        .find(|item| item.external_ref.as_deref() == Some("svc:sshd"))
        .unwrap();
        let host = CatalogStore::list_hosts(
            service.store(),
            tenant_id,
            topology_storage::Page::default(),
        )
        .unwrap()
        .into_iter()
        .find(|host| host.host_name == "node-10")
        .unwrap();
        let processes = RuntimeStore::list_process_runtime_states(
            service.store(),
            host.host_id,
            topology_storage::Page::default(),
        )
        .unwrap();
        assert_eq!(processes.len(), 1);

        let instance_id = stable_uuid(
            "service_instance",
            &format!(
                "{}:{}:{}",
                tenant_id.0, service_entity.service_id, "process:sshd:instance-a"
            ),
        );
        let instance = RuntimeStore::get_service_instance(service.store(), instance_id)
            .unwrap()
            .expect("service instance should exist");
        assert_eq!(instance.service_id, service_entity.service_id);

        let binding_id = stable_uuid(
            "runtime_binding",
            &format!(
                "{}:{}:{}",
                instance.instance_id, processes[0].process_id, "process"
            ),
        );
        let binding = RuntimeStore::get_runtime_binding(service.store(), binding_id)
            .unwrap()
            .expect("runtime binding should exist");
        assert_eq!(binding.instance_id, instance.instance_id);
        assert_eq!(binding.object_id, processes[0].process_id);
        assert!(matches!(binding.object_type, RuntimeObjectType::Process));
    }
}
