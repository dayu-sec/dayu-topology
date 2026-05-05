use chrono::{DateTime, Utc};
use topology_domain::{
    HostInventory, HostNetAssoc, NetworkDomain, NetworkSegment, ObjectKind,
    ResponsibilityAssignment, Subject, TenantId,
};
use uuid::Uuid;

use crate::{
    CatalogStore, GovernanceStore, IngestStore, Page, RuntimeStore, StorageResult,
    memory::IngestJobEntry, migrations::MIGRATIONS, not_configured, operation_failed,
};

pub mod sql {
    pub const UPSERT_HOST: &str = r#"
INSERT INTO host_inventory (
    host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
    created_at, last_inventory_at
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
ON CONFLICT (host_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    environment_id = EXCLUDED.environment_id,
    host_name = EXCLUDED.host_name,
    machine_id = EXCLUDED.machine_id,
    os_name = EXCLUDED.os_name,
    os_version = EXCLUDED.os_version,
    last_inventory_at = EXCLUDED.last_inventory_at
"#;

    pub const LIST_HOSTS: &str = r#"
SELECT host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
       created_at, last_inventory_at
FROM host_inventory
WHERE tenant_id = $1
ORDER BY host_name ASC, host_id ASC
LIMIT $2 OFFSET $3
"#;

    pub const GET_HOST: &str = r#"
SELECT host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
       created_at, last_inventory_at
FROM host_inventory
WHERE host_id = $1
"#;

    pub const UPSERT_NETWORK_DOMAIN: &str = r#"
INSERT INTO network_domain (
    network_domain_id, tenant_id, environment_id, name, kind, description, created_at, updated_at
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
ON CONFLICT (network_domain_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    environment_id = EXCLUDED.environment_id,
    name = EXCLUDED.name,
    kind = EXCLUDED.kind,
    description = EXCLUDED.description,
    updated_at = EXCLUDED.updated_at
"#;

    pub const UPSERT_NETWORK_SEGMENT: &str = r#"
INSERT INTO network_segment (
    network_segment_id, tenant_id, network_domain_id, environment_id, name, cidr, gateway_ip,
    address_family, created_at, updated_at
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
ON CONFLICT (network_segment_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    network_domain_id = EXCLUDED.network_domain_id,
    environment_id = EXCLUDED.environment_id,
    name = EXCLUDED.name,
    cidr = EXCLUDED.cidr,
    gateway_ip = EXCLUDED.gateway_ip,
    address_family = EXCLUDED.address_family,
    updated_at = EXCLUDED.updated_at
"#;

    pub const GET_NETWORK_SEGMENT: &str = r#"
SELECT network_segment_id, tenant_id, network_domain_id, environment_id, name, cidr, gateway_ip,
       address_family, created_at, updated_at
FROM network_segment
WHERE network_segment_id = $1
"#;

    pub const LIST_NETWORK_SEGMENTS: &str = r#"
SELECT network_segment_id, tenant_id, network_domain_id, environment_id, name, cidr, gateway_ip,
       address_family, created_at, updated_at
FROM network_segment
WHERE tenant_id = $1
ORDER BY name ASC, network_segment_id ASC
LIMIT $2 OFFSET $3
"#;

    pub const UPSERT_HOST_NET_ASSOC: &str = r#"
INSERT INTO host_net_assoc (
    assoc_id, tenant_id, host_id, network_segment_id, ip_addr, iface_name,
    valid_from, valid_to, created_at, updated_at
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
ON CONFLICT (assoc_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    host_id = EXCLUDED.host_id,
    network_segment_id = EXCLUDED.network_segment_id,
    ip_addr = EXCLUDED.ip_addr,
    iface_name = EXCLUDED.iface_name,
    valid_from = EXCLUDED.valid_from,
    valid_to = EXCLUDED.valid_to,
    updated_at = EXCLUDED.updated_at
"#;

    pub const LIST_HOST_NET_ASSOCS: &str = r#"
SELECT assoc_id, tenant_id, host_id, network_segment_id, ip_addr, iface_name,
       valid_from, valid_to, created_at, updated_at
FROM host_net_assoc
WHERE host_id = $1
ORDER BY created_at ASC, assoc_id ASC
LIMIT $2 OFFSET $3
"#;

    pub const UPSERT_SUBJECT: &str = r#"
INSERT INTO subject (
    subject_id, tenant_id, subject_type, display_name, external_ref, email, is_active,
    created_at, updated_at
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
ON CONFLICT (subject_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    subject_type = EXCLUDED.subject_type,
    display_name = EXCLUDED.display_name,
    external_ref = EXCLUDED.external_ref,
    email = EXCLUDED.email,
    is_active = EXCLUDED.is_active,
    updated_at = EXCLUDED.updated_at
"#;

    pub const GET_SUBJECT: &str = r#"
SELECT subject_id, tenant_id, subject_type, display_name, external_ref, email, is_active,
       created_at, updated_at
FROM subject
WHERE subject_id = $1
"#;

    pub const UPSERT_RESPONSIBILITY_ASSIGNMENT: &str = r#"
INSERT INTO responsibility_assignment (
    assignment_id, tenant_id, subject_id, target_kind, target_id, role, source,
    valid_from, valid_to, created_at, updated_at
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
ON CONFLICT (assignment_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    subject_id = EXCLUDED.subject_id,
    target_kind = EXCLUDED.target_kind,
    target_id = EXCLUDED.target_id,
    role = EXCLUDED.role,
    source = EXCLUDED.source,
    valid_from = EXCLUDED.valid_from,
    valid_to = EXCLUDED.valid_to,
    updated_at = EXCLUDED.updated_at
"#;

    pub const LIST_RESPONSIBILITY_ASSIGNMENTS_FOR_TARGET: &str = r#"
SELECT assignment_id, tenant_id, subject_id, target_kind, target_id, role, source,
       valid_from, valid_to, created_at, updated_at
FROM responsibility_assignment
WHERE target_kind = $1 AND target_id = $2
ORDER BY created_at ASC, assignment_id ASC
LIMIT $3 OFFSET $4
"#;

    pub const UPSERT_INGEST_JOB: &str = r#"
INSERT INTO ingest_job (
    ingest_id, tenant_id, source_kind, source_name, received_at, status, payload_ref, error
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
ON CONFLICT (ingest_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    source_kind = EXCLUDED.source_kind,
    source_name = EXCLUDED.source_name,
    received_at = EXCLUDED.received_at,
    status = EXCLUDED.status,
    payload_ref = EXCLUDED.payload_ref,
    error = EXCLUDED.error
"#;

    pub const GET_INGEST_JOB: &str = r#"
SELECT ingest_id, tenant_id, source_kind, source_name, received_at, status, payload_ref, error
FROM ingest_job
WHERE ingest_id = $1
"#;
}

pub trait PostgresExecutor: Clone {
    fn exec(&self, sql: &str, params: &[String]) -> StorageResult<u64>;
    fn query_rows(&self, sql: &str, params: &[String]) -> StorageResult<Vec<Vec<String>>>;
}

#[derive(Debug, Clone)]
pub struct PostgresTopologyStore<E> {
    executor: E,
}

impl<E> PostgresTopologyStore<E>
where
    E: PostgresExecutor,
{
    pub fn new(executor: E) -> Self {
        Self { executor }
    }

    pub fn run_migrations(&self) -> StorageResult<()> {
        for migration in MIGRATIONS {
            self.executor.exec(migration.sql, &[])?;
        }
        Ok(())
    }
}

impl<E> CatalogStore for PostgresTopologyStore<E>
where
    E: PostgresExecutor,
{
    fn upsert_business(&self, _business: &topology_domain::BusinessDomain) -> StorageResult<()> {
        Err(not_configured())
    }

    fn get_business(
        &self,
        _business_id: Uuid,
    ) -> StorageResult<Option<topology_domain::BusinessDomain>> {
        Ok(None)
    }

    fn list_businesses(
        &self,
        _tenant_id: TenantId,
        _page: Page,
    ) -> StorageResult<Vec<topology_domain::BusinessDomain>> {
        Ok(Vec::new())
    }

    fn upsert_system(&self, _system: &topology_domain::SystemBoundary) -> StorageResult<()> {
        Err(not_configured())
    }

    fn get_system(
        &self,
        _system_id: Uuid,
    ) -> StorageResult<Option<topology_domain::SystemBoundary>> {
        Ok(None)
    }

    fn upsert_subsystem(&self, _subsystem: &topology_domain::Subsystem) -> StorageResult<()> {
        Err(not_configured())
    }

    fn get_subsystem(
        &self,
        _subsystem_id: Uuid,
    ) -> StorageResult<Option<topology_domain::Subsystem>> {
        Ok(None)
    }

    fn upsert_service(&self, _service: &topology_domain::ServiceEntity) -> StorageResult<()> {
        Err(not_configured())
    }

    fn get_service(
        &self,
        _service_id: Uuid,
    ) -> StorageResult<Option<topology_domain::ServiceEntity>> {
        Ok(None)
    }

    fn list_services(
        &self,
        _tenant_id: TenantId,
        _page: Page,
    ) -> StorageResult<Vec<topology_domain::ServiceEntity>> {
        Ok(Vec::new())
    }

    fn upsert_cluster(&self, _cluster: &topology_domain::ClusterInventory) -> StorageResult<()> {
        Err(not_configured())
    }

    fn get_cluster(
        &self,
        _cluster_id: Uuid,
    ) -> StorageResult<Option<topology_domain::ClusterInventory>> {
        Ok(None)
    }

    fn upsert_namespace(
        &self,
        _namespace: &topology_domain::NamespaceInventory,
    ) -> StorageResult<()> {
        Err(not_configured())
    }

    fn get_namespace(
        &self,
        _namespace_id: Uuid,
    ) -> StorageResult<Option<topology_domain::NamespaceInventory>> {
        Ok(None)
    }

    fn upsert_workload(&self, _workload: &topology_domain::WorkloadEntity) -> StorageResult<()> {
        Err(not_configured())
    }

    fn get_workload(
        &self,
        _workload_id: Uuid,
    ) -> StorageResult<Option<topology_domain::WorkloadEntity>> {
        Ok(None)
    }

    fn upsert_pod(&self, _pod: &topology_domain::PodInventory) -> StorageResult<()> {
        Err(not_configured())
    }

    fn get_pod(&self, _pod_id: Uuid) -> StorageResult<Option<topology_domain::PodInventory>> {
        Ok(None)
    }

    fn upsert_host(&self, host: &HostInventory) -> StorageResult<()> {
        self.executor.exec(
            sql::UPSERT_HOST,
            &[
                host.host_id.to_string(),
                host.tenant_id.0.to_string(),
                host.environment_id
                    .map(|id| id.0.to_string())
                    .unwrap_or_default(),
                host.host_name.clone(),
                host.machine_id.clone().unwrap_or_default(),
                host.os_name.clone().unwrap_or_default(),
                host.os_version.clone().unwrap_or_default(),
                host.created_at.to_rfc3339(),
                host.last_inventory_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn get_host(&self, host_id: Uuid) -> StorageResult<Option<HostInventory>> {
        let rows = self
            .executor
            .query_rows(sql::GET_HOST, &[host_id.to_string()])?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_host(&row))
            .transpose()?)
    }

    fn list_hosts(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<HostInventory>> {
        let rows = self.executor.query_rows(
            sql::LIST_HOSTS,
            &[
                tenant_id.0.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter().map(|row| decode_host(&row)).collect()
    }

    fn upsert_network_domain(&self, domain: &NetworkDomain) -> StorageResult<()> {
        self.executor.exec(
            sql::UPSERT_NETWORK_DOMAIN,
            &[
                domain.network_domain_id.to_string(),
                domain.tenant_id.0.to_string(),
                domain
                    .environment_id
                    .map(|id| id.0.to_string())
                    .unwrap_or_default(),
                domain.name.clone(),
                format!("{:?}", domain.kind),
                domain.description.clone().unwrap_or_default(),
                domain.created_at.to_rfc3339(),
                domain.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn get_network_domain(&self, network_domain_id: Uuid) -> StorageResult<Option<NetworkDomain>> {
        let rows = self.executor.query_rows(
            "SELECT network_domain_id, tenant_id, environment_id, name, kind, description, created_at, updated_at FROM network_domain WHERE network_domain_id = $1",
            &[network_domain_id.to_string()],
        )?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_network_domain(&row))
            .transpose()?)
    }

    fn upsert_network_segment(&self, segment: &NetworkSegment) -> StorageResult<()> {
        self.executor.exec(
            sql::UPSERT_NETWORK_SEGMENT,
            &[
                segment.network_segment_id.to_string(),
                segment.tenant_id.0.to_string(),
                segment
                    .network_domain_id
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
                segment
                    .environment_id
                    .map(|id| id.0.to_string())
                    .unwrap_or_default(),
                segment.name.clone(),
                segment.cidr.clone().unwrap_or_default(),
                segment.gateway_ip.clone().unwrap_or_default(),
                format!("{:?}", segment.address_family),
                segment.created_at.to_rfc3339(),
                segment.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn get_network_segment(
        &self,
        network_segment_id: Uuid,
    ) -> StorageResult<Option<NetworkSegment>> {
        let rows = self
            .executor
            .query_rows(sql::GET_NETWORK_SEGMENT, &[network_segment_id.to_string()])?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_network_segment(&row))
            .transpose()?)
    }

    fn list_network_segments(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<NetworkSegment>> {
        let rows = self.executor.query_rows(
            sql::LIST_NETWORK_SEGMENTS,
            &[
                tenant_id.0.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter()
            .map(|row| decode_network_segment(&row))
            .collect()
    }

    fn upsert_subject(&self, subject: &Subject) -> StorageResult<()> {
        self.executor.exec(
            sql::UPSERT_SUBJECT,
            &[
                subject.subject_id.to_string(),
                subject.tenant_id.0.to_string(),
                format!("{:?}", subject.subject_type),
                subject.display_name.clone(),
                subject.external_ref.clone().unwrap_or_default(),
                subject.email.clone().unwrap_or_default(),
                subject.is_active.to_string(),
                subject.created_at.to_rfc3339(),
                subject.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn get_subject(&self, subject_id: Uuid) -> StorageResult<Option<Subject>> {
        let rows = self
            .executor
            .query_rows(sql::GET_SUBJECT, &[subject_id.to_string()])?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_subject(&row))
            .transpose()?)
    }

    fn list_subjects(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<Subject>> {
        let rows = self.executor.query_rows(
            "SELECT subject_id, tenant_id, subject_type, display_name, external_ref, email, is_active, created_at, updated_at FROM subject WHERE tenant_id = $1 ORDER BY display_name ASC, subject_id ASC LIMIT $2 OFFSET $3",
            &[
                tenant_id.0.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter().map(|row| decode_subject(&row)).collect()
    }
}

impl<E> RuntimeStore for PostgresTopologyStore<E>
where
    E: PostgresExecutor,
{
    fn insert_host_runtime_state(
        &self,
        _state: &topology_domain::HostRuntimeState,
    ) -> StorageResult<()> {
        Err(not_configured())
    }

    fn list_host_runtime_states(
        &self,
        _host_id: Uuid,
        _page: Page,
    ) -> StorageResult<Vec<topology_domain::HostRuntimeState>> {
        Ok(Vec::new())
    }

    fn upsert_service_instance(
        &self,
        _instance: &topology_domain::ServiceInstance,
    ) -> StorageResult<()> {
        Err(not_configured())
    }

    fn get_service_instance(
        &self,
        _instance_id: Uuid,
    ) -> StorageResult<Option<topology_domain::ServiceInstance>> {
        Ok(None)
    }

    fn upsert_runtime_binding(
        &self,
        _binding: &topology_domain::RuntimeBinding,
    ) -> StorageResult<()> {
        Err(not_configured())
    }

    fn get_runtime_binding(
        &self,
        _binding_id: Uuid,
    ) -> StorageResult<Option<topology_domain::RuntimeBinding>> {
        Ok(None)
    }

    fn list_runtime_bindings_for_instance(
        &self,
        _instance_id: Uuid,
        _page: Page,
    ) -> StorageResult<Vec<topology_domain::RuntimeBinding>> {
        Ok(Vec::new())
    }

    fn upsert_workload_pod_membership(
        &self,
        _membership: &topology_domain::WorkloadPodMembership,
    ) -> StorageResult<()> {
        Err(not_configured())
    }

    fn upsert_pod_placement(
        &self,
        _placement: &topology_domain::PodPlacement,
    ) -> StorageResult<()> {
        Err(not_configured())
    }

    fn upsert_host_net_assoc(&self, assoc: &HostNetAssoc) -> StorageResult<()> {
        self.executor.exec(
            sql::UPSERT_HOST_NET_ASSOC,
            &[
                assoc.assoc_id.to_string(),
                assoc.tenant_id.0.to_string(),
                assoc.host_id.to_string(),
                assoc.network_segment_id.to_string(),
                assoc.ip_addr.clone(),
                assoc.iface_name.clone().unwrap_or_default(),
                assoc.validity.valid_from.to_rfc3339(),
                assoc
                    .validity
                    .valid_to
                    .map(|value| value.to_rfc3339())
                    .unwrap_or_default(),
                assoc.created_at.to_rfc3339(),
                assoc.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn list_host_net_assocs(&self, host_id: Uuid, page: Page) -> StorageResult<Vec<HostNetAssoc>> {
        let rows = self.executor.query_rows(
            sql::LIST_HOST_NET_ASSOCS,
            &[
                host_id.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter()
            .map(|row| decode_host_net_assoc(&row))
            .collect()
    }
}

impl<E> GovernanceStore for PostgresTopologyStore<E>
where
    E: PostgresExecutor,
{
    fn upsert_responsibility_assignment(
        &self,
        assignment: &ResponsibilityAssignment,
    ) -> StorageResult<()> {
        self.executor.exec(
            sql::UPSERT_RESPONSIBILITY_ASSIGNMENT,
            &[
                assignment.assignment_id.to_string(),
                assignment.tenant_id.0.to_string(),
                assignment.subject_id.to_string(),
                format!("{:?}", assignment.target_kind),
                assignment.target_id.to_string(),
                format!("{:?}", assignment.role),
                assignment.source.clone(),
                assignment.validity.valid_from.to_rfc3339(),
                assignment
                    .validity
                    .valid_to
                    .map(|value| value.to_rfc3339())
                    .unwrap_or_default(),
                assignment.created_at.to_rfc3339(),
                assignment.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn get_responsibility_assignment(
        &self,
        assignment_id: Uuid,
    ) -> StorageResult<Option<ResponsibilityAssignment>> {
        let rows = self.executor.query_rows(
            "SELECT assignment_id, tenant_id, subject_id, target_kind, target_id, role, source, valid_from, valid_to, created_at, updated_at FROM responsibility_assignment WHERE assignment_id = $1",
            &[assignment_id.to_string()],
        )?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_responsibility_assignment(&row))
            .transpose()?)
    }

    fn list_responsibility_assignments_for_target(
        &self,
        target_kind: ObjectKind,
        target_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ResponsibilityAssignment>> {
        let rows = self.executor.query_rows(
            sql::LIST_RESPONSIBILITY_ASSIGNMENTS_FOR_TARGET,
            &[
                format!("{:?}", target_kind),
                target_id.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter()
            .map(|row| decode_responsibility_assignment(&row))
            .collect()
    }
}

impl<E> IngestStore for PostgresTopologyStore<E>
where
    E: PostgresExecutor,
{
    fn record_ingest_job(&self, entry: IngestJobEntry) -> StorageResult<()> {
        self.executor.exec(
            sql::UPSERT_INGEST_JOB,
            &[
                entry.ingest_id,
                entry.tenant_id.0.to_string(),
                entry.source_kind,
                entry.source_name,
                entry.received_at.to_rfc3339(),
                entry.status,
                entry.payload_ref.unwrap_or_default(),
                entry.error.unwrap_or_default(),
            ],
        )?;
        Ok(())
    }

    fn get_ingest_job(&self, ingest_id: &str) -> StorageResult<Option<IngestJobEntry>> {
        let rows = self
            .executor
            .query_rows(sql::GET_INGEST_JOB, &[ingest_id.to_string()])?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_ingest_job(&row))
            .transpose()?)
    }
}

#[derive(Debug, Clone, Default)]
pub struct RecordingExecutor {
    calls: std::sync::Arc<std::sync::Mutex<Vec<(String, Vec<String>)>>>,
}

impl RecordingExecutor {
    pub fn calls(&self) -> Vec<(String, Vec<String>)> {
        self.calls
            .lock()
            .expect("recording executor poisoned")
            .clone()
    }
}

#[derive(Debug, Clone, Default)]
pub struct MemoryPostgresExecutor {
    state: std::sync::Arc<std::sync::Mutex<MemoryPostgresState>>,
}

#[derive(Debug, Default)]
struct MemoryPostgresState {
    hosts: std::collections::BTreeMap<String, Vec<String>>,
    network_domains: std::collections::BTreeMap<String, Vec<String>>,
    network_segments: std::collections::BTreeMap<String, Vec<String>>,
    host_net_assocs: std::collections::BTreeMap<String, Vec<String>>,
    subjects: std::collections::BTreeMap<String, Vec<String>>,
    responsibility_assignments: std::collections::BTreeMap<String, Vec<String>>,
    ingest_jobs: std::collections::BTreeMap<String, Vec<String>>,
}

impl PostgresExecutor for RecordingExecutor {
    fn exec(&self, sql: &str, params: &[String]) -> StorageResult<u64> {
        self.calls
            .lock()
            .map_err(|err| operation_failed(err.to_string()))?
            .push((sql.to_string(), params.to_vec()));
        Ok(1)
    }

    fn query_rows(&self, sql: &str, params: &[String]) -> StorageResult<Vec<Vec<String>>> {
        self.calls
            .lock()
            .map_err(|err| operation_failed(err.to_string()))?
            .push((sql.to_string(), params.to_vec()));
        Ok(Vec::new())
    }
}

impl PostgresExecutor for MemoryPostgresExecutor {
    fn exec(&self, sql: &str, params: &[String]) -> StorageResult<u64> {
        let mut state = self
            .state
            .lock()
            .map_err(|err| operation_failed(err.to_string()))?;

        match sql {
            value if value == sql::UPSERT_HOST => {
                state.hosts.insert(params[0].clone(), params.to_vec());
            }
            value if value == sql::UPSERT_NETWORK_DOMAIN => {
                state
                    .network_domains
                    .insert(params[0].clone(), params.to_vec());
            }
            value if value == sql::UPSERT_NETWORK_SEGMENT => {
                state
                    .network_segments
                    .insert(params[0].clone(), params.to_vec());
            }
            value if value == sql::UPSERT_HOST_NET_ASSOC => {
                state
                    .host_net_assocs
                    .insert(params[0].clone(), params.to_vec());
            }
            value if value == sql::UPSERT_SUBJECT => {
                state.subjects.insert(params[0].clone(), params.to_vec());
            }
            value if value == sql::UPSERT_RESPONSIBILITY_ASSIGNMENT => {
                state
                    .responsibility_assignments
                    .insert(params[0].clone(), params.to_vec());
            }
            value if value == sql::UPSERT_INGEST_JOB => {
                state.ingest_jobs.insert(params[0].clone(), params.to_vec());
            }
            _ => {}
        }

        Ok(1)
    }

    fn query_rows(&self, sql: &str, params: &[String]) -> StorageResult<Vec<Vec<String>>> {
        let state = self
            .state
            .lock()
            .map_err(|err| operation_failed(err.to_string()))?;

        let rows = match sql {
            value if value == sql::GET_HOST => {
                state.hosts.get(&params[0]).into_iter().cloned().collect()
            }
            value if value == sql::LIST_HOSTS => state
                .hosts
                .values()
                .filter(|row| row[1] == params[0])
                .cloned()
                .collect(),
            value if value == sql::GET_NETWORK_SEGMENT => state
                .network_segments
                .get(&params[0])
                .into_iter()
                .cloned()
                .collect(),
            value if value == sql::LIST_NETWORK_SEGMENTS => state
                .network_segments
                .values()
                .filter(|row| row[1] == params[0])
                .cloned()
                .collect(),
            value if value == sql::LIST_HOST_NET_ASSOCS => state
                .host_net_assocs
                .values()
                .filter(|row| row[2] == params[0])
                .cloned()
                .collect(),
            value if value == sql::GET_SUBJECT => state
                .subjects
                .get(&params[0])
                .into_iter()
                .cloned()
                .collect(),
            value if value == sql::GET_INGEST_JOB => state
                .ingest_jobs
                .get(&params[0])
                .into_iter()
                .cloned()
                .collect(),
            value if value == sql::LIST_RESPONSIBILITY_ASSIGNMENTS_FOR_TARGET => state
                .responsibility_assignments
                .values()
                .filter(|row| row[3] == params[0] && row[4] == params[1])
                .cloned()
                .collect(),
            value if value.contains("FROM network_domain WHERE network_domain_id = $1") => state
                .network_domains
                .get(&params[0])
                .into_iter()
                .cloned()
                .collect(),
            value if value.contains("FROM subject WHERE tenant_id = $1") => state
                .subjects
                .values()
                .filter(|row| row[1] == params[0])
                .cloned()
                .collect(),
            value if value.contains("FROM responsibility_assignment WHERE assignment_id = $1") => {
                state
                    .responsibility_assignments
                    .get(&params[0])
                    .into_iter()
                    .cloned()
                    .collect()
            }
            _ => Vec::new(),
        };

        Ok(rows)
    }
}

fn decode_host(row: &[String]) -> StorageResult<HostInventory> {
    Ok(HostInventory {
        host_id: parse_uuid(&row[0])?,
        tenant_id: TenantId(parse_uuid(&row[1])?),
        environment_id: parse_optional_uuid(&row[2])?.map(topology_domain::EnvironmentId),
        host_name: row[3].clone(),
        machine_id: empty_to_none(&row[4]),
        os_name: empty_to_none(&row[5]),
        os_version: empty_to_none(&row[6]),
        created_at: parse_datetime(&row[7])?,
        last_inventory_at: parse_datetime(&row[8])?,
    })
}

fn decode_network_domain(row: &[String]) -> StorageResult<NetworkDomain> {
    Ok(NetworkDomain {
        network_domain_id: parse_uuid(&row[0])?,
        tenant_id: TenantId(parse_uuid(&row[1])?),
        environment_id: parse_optional_uuid(&row[2])?.map(topology_domain::EnvironmentId),
        name: row[3].clone(),
        kind: parse_network_domain_kind(&row[4])?,
        description: empty_to_none(&row[5]),
        created_at: parse_datetime(&row[6])?,
        updated_at: parse_datetime(&row[7])?,
    })
}

fn decode_network_segment(row: &[String]) -> StorageResult<NetworkSegment> {
    Ok(NetworkSegment {
        network_segment_id: parse_uuid(&row[0])?,
        tenant_id: TenantId(parse_uuid(&row[1])?),
        network_domain_id: parse_optional_uuid(&row[2])?,
        environment_id: parse_optional_uuid(&row[3])?.map(topology_domain::EnvironmentId),
        name: row[4].clone(),
        cidr: empty_to_none(&row[5]),
        gateway_ip: empty_to_none(&row[6]),
        address_family: parse_address_family(&row[7])?,
        created_at: parse_datetime(&row[8])?,
        updated_at: parse_datetime(&row[9])?,
    })
}

fn decode_host_net_assoc(row: &[String]) -> StorageResult<HostNetAssoc> {
    Ok(HostNetAssoc {
        assoc_id: parse_uuid(&row[0])?,
        tenant_id: TenantId(parse_uuid(&row[1])?),
        host_id: parse_uuid(&row[2])?,
        network_segment_id: parse_uuid(&row[3])?,
        ip_addr: row[4].clone(),
        iface_name: empty_to_none(&row[5]),
        validity: topology_domain::ValidityWindow {
            valid_from: parse_datetime(&row[6])?,
            valid_to: empty_to_none(&row[7])
                .map(|value| parse_datetime(&value))
                .transpose()?,
        },
        created_at: parse_datetime(&row[8])?,
        updated_at: parse_datetime(&row[9])?,
    })
}

fn decode_subject(row: &[String]) -> StorageResult<Subject> {
    Ok(Subject {
        subject_id: parse_uuid(&row[0])?,
        tenant_id: TenantId(parse_uuid(&row[1])?),
        subject_type: parse_subject_type(&row[2])?,
        display_name: row[3].clone(),
        external_ref: empty_to_none(&row[4]),
        email: empty_to_none(&row[5]),
        is_active: row[6] == "true",
        created_at: parse_datetime(&row[7])?,
        updated_at: parse_datetime(&row[8])?,
    })
}

fn decode_responsibility_assignment(row: &[String]) -> StorageResult<ResponsibilityAssignment> {
    Ok(ResponsibilityAssignment {
        assignment_id: parse_uuid(&row[0])?,
        tenant_id: TenantId(parse_uuid(&row[1])?),
        subject_id: parse_uuid(&row[2])?,
        target_kind: parse_object_kind(&row[3])?,
        target_id: parse_uuid(&row[4])?,
        role: parse_responsibility_role(&row[5])?,
        source: row[6].clone(),
        validity: topology_domain::ValidityWindow {
            valid_from: parse_datetime(&row[7])?,
            valid_to: empty_to_none(&row[8])
                .map(|value| parse_datetime(&value))
                .transpose()?,
        },
        created_at: parse_datetime(&row[9])?,
        updated_at: parse_datetime(&row[10])?,
    })
}

fn decode_ingest_job(row: &[String]) -> StorageResult<IngestJobEntry> {
    Ok(IngestJobEntry {
        ingest_id: row[0].clone(),
        tenant_id: TenantId(parse_uuid(&row[1])?),
        source_kind: row[2].clone(),
        source_name: row[3].clone(),
        received_at: parse_datetime(&row[4])?,
        status: row[5].clone(),
        payload_ref: empty_to_none(&row[6]),
        error: empty_to_none(&row[7]),
    })
}

fn parse_uuid(value: &str) -> StorageResult<Uuid> {
    Uuid::parse_str(value).map_err(|err| operation_failed(err.to_string()))
}

fn parse_optional_uuid(value: &str) -> StorageResult<Option<Uuid>> {
    if value.is_empty() {
        Ok(None)
    } else {
        parse_uuid(value).map(Some)
    }
}

fn parse_datetime(value: &str) -> StorageResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|err| operation_failed(err.to_string()))
}

fn empty_to_none(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn parse_network_domain_kind(value: &str) -> StorageResult<topology_domain::NetworkDomainKind> {
    match value {
        "Lan" => Ok(topology_domain::NetworkDomainKind::Lan),
        "Wan" => Ok(topology_domain::NetworkDomainKind::Wan),
        "Vpc" => Ok(topology_domain::NetworkDomainKind::Vpc),
        "Vnet" => Ok(topology_domain::NetworkDomainKind::Vnet),
        "Vlan" => Ok(topology_domain::NetworkDomainKind::Vlan),
        "Overlay" => Ok(topology_domain::NetworkDomainKind::Overlay),
        "Unknown" => Ok(topology_domain::NetworkDomainKind::Unknown),
        other => Err(operation_failed(format!(
            "unsupported network domain kind: {other}"
        ))),
    }
}

fn parse_address_family(value: &str) -> StorageResult<topology_domain::AddressFamily> {
    match value {
        "Ipv4" => Ok(topology_domain::AddressFamily::Ipv4),
        "Ipv6" => Ok(topology_domain::AddressFamily::Ipv6),
        other => Err(operation_failed(format!(
            "unsupported address family: {other}"
        ))),
    }
}

fn parse_subject_type(value: &str) -> StorageResult<topology_domain::SubjectType> {
    match value {
        "User" => Ok(topology_domain::SubjectType::User),
        "Team" => Ok(topology_domain::SubjectType::Team),
        "Rotation" => Ok(topology_domain::SubjectType::Rotation),
        "ServiceAccount" => Ok(topology_domain::SubjectType::ServiceAccount),
        other => Err(operation_failed(format!(
            "unsupported subject type: {other}"
        ))),
    }
}

fn parse_object_kind(value: &str) -> StorageResult<ObjectKind> {
    match value {
        "Host" => Ok(ObjectKind::Host),
        "NetworkSegment" => Ok(ObjectKind::NetworkSegment),
        "Subject" => Ok(ObjectKind::Subject),
        other => Err(operation_failed(format!(
            "unsupported object kind: {other}"
        ))),
    }
}

fn parse_responsibility_role(value: &str) -> StorageResult<topology_domain::ResponsibilityRole> {
    match value {
        "Owner" => Ok(topology_domain::ResponsibilityRole::Owner),
        "Maintainer" => Ok(topology_domain::ResponsibilityRole::Maintainer),
        "Oncall" => Ok(topology_domain::ResponsibilityRole::Oncall),
        "Security" => Ok(topology_domain::ResponsibilityRole::Security),
        other => Err(operation_failed(format!(
            "unsupported responsibility role: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{sql, *};

    #[test]
    fn core_upserts_are_idempotent() {
        for statement in [
            sql::UPSERT_HOST,
            sql::UPSERT_NETWORK_DOMAIN,
            sql::UPSERT_NETWORK_SEGMENT,
            sql::UPSERT_HOST_NET_ASSOC,
            sql::UPSERT_SUBJECT,
            sql::UPSERT_RESPONSIBILITY_ASSIGNMENT,
            sql::UPSERT_INGEST_JOB,
        ] {
            assert!(
                statement.contains("ON CONFLICT"),
                "upsert statement must be idempotent: {statement}"
            );
        }
    }

    #[test]
    fn core_queries_include_pagination_where_expected() {
        for statement in [
            sql::LIST_HOSTS,
            sql::LIST_NETWORK_SEGMENTS,
            sql::LIST_HOST_NET_ASSOCS,
            sql::LIST_RESPONSIBILITY_ASSIGNMENTS_FOR_TARGET,
        ] {
            assert!(statement.contains("LIMIT"));
        }
    }

    #[test]
    fn postgres_store_runs_migrations_and_records_exec_calls() {
        let executor = RecordingExecutor::default();
        let store = PostgresTopologyStore::new(executor.clone());

        store.run_migrations().unwrap();

        let calls = executor.calls();
        assert!(!calls.is_empty());
        assert!(
            calls[0]
                .0
                .contains("CREATE TABLE IF NOT EXISTS schema_migrations")
        );
    }

    #[test]
    fn postgres_store_uses_ingest_job_upsert_sql() {
        let executor = RecordingExecutor::default();
        let store = PostgresTopologyStore::new(executor.clone());

        store
            .record_ingest_job(IngestJobEntry {
                ingest_id: "ing-1".to_string(),
                tenant_id: TenantId(Uuid::new_v4()),
                source_name: "demo".to_string(),
                source_kind: "batch_import".to_string(),
                received_at: Utc::now(),
                status: "accepted".to_string(),
                payload_ref: None,
                error: None,
            })
            .unwrap();

        let calls = executor.calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].0.contains("INSERT INTO ingest_job"));
    }

    #[test]
    fn memory_postgres_executor_round_trips_minimal_p0_records() {
        let executor = MemoryPostgresExecutor::default();
        let store = PostgresTopologyStore::new(executor);
        let tenant_id = TenantId(Uuid::new_v4());
        let now = Utc::now();
        let host = HostInventory {
            host_id: Uuid::new_v4(),
            tenant_id,
            environment_id: None,
            host_name: "node-01".to_string(),
            machine_id: Some("machine-01".to_string()),
            os_name: Some("linux".to_string()),
            os_version: Some("6.8".to_string()),
            created_at: now,
            last_inventory_at: now,
        };
        let domain = NetworkDomain {
            network_domain_id: Uuid::new_v4(),
            tenant_id,
            environment_id: None,
            name: "default".to_string(),
            kind: topology_domain::NetworkDomainKind::Unknown,
            description: None,
            created_at: now,
            updated_at: now,
        };
        let segment = NetworkSegment {
            network_segment_id: Uuid::new_v4(),
            tenant_id,
            network_domain_id: Some(domain.network_domain_id),
            environment_id: None,
            name: "10.0.0.0/24".to_string(),
            cidr: Some("10.0.0.0/24".to_string()),
            gateway_ip: Some("10.0.0.1".to_string()),
            address_family: topology_domain::AddressFamily::Ipv4,
            created_at: now,
            updated_at: now,
        };
        let subject = Subject {
            subject_id: Uuid::new_v4(),
            tenant_id,
            subject_type: topology_domain::SubjectType::User,
            display_name: "alice".to_string(),
            external_ref: None,
            email: Some("alice@example.com".to_string()),
            is_active: true,
            created_at: now,
            updated_at: now,
        };
        let assignment = ResponsibilityAssignment {
            assignment_id: Uuid::new_v4(),
            tenant_id,
            subject_id: subject.subject_id,
            target_kind: ObjectKind::Host,
            target_id: host.host_id,
            role: topology_domain::ResponsibilityRole::Owner,
            source: "batch_import".to_string(),
            validity: topology_domain::ValidityWindow {
                valid_from: now,
                valid_to: None,
            },
            created_at: now,
            updated_at: now,
        };

        store.upsert_host(&host).unwrap();
        store.upsert_network_domain(&domain).unwrap();
        store.upsert_network_segment(&segment).unwrap();
        store.upsert_subject(&subject).unwrap();
        store.upsert_responsibility_assignment(&assignment).unwrap();
        store
            .record_ingest_job(IngestJobEntry {
                ingest_id: "ing-1".to_string(),
                tenant_id,
                source_name: "demo".to_string(),
                source_kind: "batch_import".to_string(),
                received_at: now,
                status: "accepted".to_string(),
                payload_ref: None,
                error: None,
            })
            .unwrap();

        assert!(store.get_host(host.host_id).unwrap().is_some());
        assert_eq!(
            store.list_hosts(tenant_id, Page::default()).unwrap().len(),
            1
        );
        assert!(
            store
                .get_network_domain(domain.network_domain_id)
                .unwrap()
                .is_some()
        );
        assert!(
            store
                .get_network_segment(segment.network_segment_id)
                .unwrap()
                .is_some()
        );
        assert!(store.get_subject(subject.subject_id).unwrap().is_some());
        assert_eq!(
            store
                .list_responsibility_assignments_for_target(
                    ObjectKind::Host,
                    host.host_id,
                    Page::default(),
                )
                .unwrap()
                .len(),
            1
        );
        assert!(store.get_ingest_job("ing-1").unwrap().is_some());
    }
}
