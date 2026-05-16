use chrono::{DateTime, Utc};
use orion_error::prelude::*;
use std::sync::Arc;
use tokio_postgres::{NoTls, types::ToSql};
use topology_domain::{
    HostInventory, HostNetAssoc, HostRuntimeState, NetworkDomain, NetworkSegment, ObjectKind,
    ProcessRuntimeState, ResponsibilityAssignment, RuntimeBinding, RuntimeObjectType,
    ServiceEntity, ServiceInstance, Subject, TenantId,
};
use uuid::Uuid;

use crate::{
    CatalogStore, GovernanceStore, IngestStore, Page, RuntimeStore, StorageReason, StorageResult,
    decode_failed, lock_failed, memory::IngestJobEntry, migrations::MIGRATIONS, not_configured,
    AsyncCatalogStore, AsyncGovernanceStore, AsyncRuntimeStore,
    memory::{AsyncIngestStore, },
     operation_failed,
};

pub mod sql {
    pub const RESET_PUBLIC_SCHEMA: &str = r#"
DROP SCHEMA public CASCADE;
CREATE SCHEMA public;
"#;

    pub const UPSERT_HOST: &str = r#"
INSERT INTO host_inventory (
    host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
    created_at, last_inventory_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid, $4,
    NULLIF($5, ''), NULLIF($6, ''), NULLIF($7, ''),
    NULLIF($8, '')::timestamptz, NULLIF($9, '')::timestamptz
)
ON CONFLICT (machine_id) WHERE machine_id IS NOT NULL DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    environment_id = EXCLUDED.environment_id,
    host_name = EXCLUDED.host_name,
    machine_id = EXCLUDED.machine_id,
    os_name = EXCLUDED.os_name,
    os_version = EXCLUDED.os_version,
    last_inventory_at = EXCLUDED.last_inventory_at
"#;

    pub const UPSERT_HOST_WITHOUT_MACHINE_ID: &str = r#"
INSERT INTO host_inventory (
    host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
    created_at, last_inventory_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid, $4,
    NULLIF($5, ''), NULLIF($6, ''), NULLIF($7, ''),
    NULLIF($8, '')::timestamptz, NULLIF($9, '')::timestamptz
)
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
WHERE tenant_id = NULLIF($1, '')::uuid
ORDER BY host_name ASC, host_id ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

    pub const LIST_ALL_HOSTS: &str = r#"
SELECT host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
       created_at, last_inventory_at
FROM host_inventory
ORDER BY host_name ASC, host_id ASC
LIMIT NULLIF($1, '')::int4 OFFSET NULLIF($2, '')::int4
"#;

    pub const GET_HOST: &str = r#"
SELECT host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
       created_at, last_inventory_at
FROM host_inventory
WHERE host_id = NULLIF($1, '')::uuid
"#;

    pub const UPSERT_SERVICE: &str = r#"
INSERT INTO service_entity (
    service_id, tenant_id, business_id, system_id, subsystem_id, name, namespace,
    service_type, boundary, provider, external_ref, created_at, updated_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid,
    NULLIF($4, '')::uuid, NULLIF($5, '')::uuid, $6, NULLIF($7, ''), $8, $9,
    NULLIF($10, ''), NULLIF($11, ''), NULLIF($12, '')::timestamptz,
    NULLIF($13, '')::timestamptz
)
ON CONFLICT (service_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    business_id = EXCLUDED.business_id,
    system_id = EXCLUDED.system_id,
    subsystem_id = EXCLUDED.subsystem_id,
    name = EXCLUDED.name,
    namespace = EXCLUDED.namespace,
    service_type = EXCLUDED.service_type,
    boundary = EXCLUDED.boundary,
    provider = EXCLUDED.provider,
    external_ref = EXCLUDED.external_ref,
    updated_at = EXCLUDED.updated_at
"#;

    pub const GET_SERVICE: &str = r#"
SELECT service_id, tenant_id, business_id, system_id, subsystem_id, name, namespace,
       service_type, boundary, provider, external_ref, created_at, updated_at
FROM service_entity
WHERE service_id = NULLIF($1, '')::uuid
"#;

    pub const LIST_SERVICES: &str = r#"
SELECT service_id, tenant_id, business_id, system_id, subsystem_id, name, namespace,
       service_type, boundary, provider, external_ref, created_at, updated_at
FROM service_entity
WHERE tenant_id = NULLIF($1, '')::uuid
ORDER BY name ASC, service_id ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

    pub const UPSERT_NETWORK_DOMAIN: &str = r#"
INSERT INTO network_domain (
    network_domain_id, tenant_id, environment_id, name, kind, description, created_at, updated_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid, $4, $5,
    NULLIF($6, ''), NULLIF($7, '')::timestamptz, NULLIF($8, '')::timestamptz
)
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
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid,
    NULLIF($4, '')::uuid, $5, NULLIF($6, ''), NULLIF($7, ''), $8,
    NULLIF($9, '')::timestamptz, NULLIF($10, '')::timestamptz
)
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
WHERE network_segment_id = NULLIF($1, '')::uuid
"#;

    pub const LIST_NETWORK_SEGMENTS: &str = r#"
SELECT network_segment_id, tenant_id, network_domain_id, environment_id, name, cidr, gateway_ip,
       address_family, created_at, updated_at
FROM network_segment
WHERE tenant_id = NULLIF($1, '')::uuid
ORDER BY name ASC, network_segment_id ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

    pub const UPSERT_HOST_NET_ASSOC: &str = r#"
INSERT INTO host_net_assoc (
    assoc_id, tenant_id, host_id, network_segment_id, ip_addr, iface_name,
    valid_from, valid_to, created_at, updated_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid,
    NULLIF($4, '')::uuid, $5, NULLIF($6, ''), NULLIF($7, '')::timestamptz,
    NULLIF($8, '')::timestamptz, NULLIF($9, '')::timestamptz,
    NULLIF($10, '')::timestamptz
)
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
WHERE host_id = NULLIF($1, '')::uuid
ORDER BY created_at ASC, assoc_id ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

    pub const UPSERT_SUBJECT: &str = r#"
INSERT INTO subject (
    subject_id, tenant_id, subject_type, display_name, external_ref, email, is_active,
    created_at, updated_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, $3, $4, NULLIF($5, ''),
    NULLIF($6, ''), NULLIF($7, '')::boolean, NULLIF($8, '')::timestamptz,
    NULLIF($9, '')::timestamptz
)
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
WHERE subject_id = NULLIF($1, '')::uuid
"#;

    pub const UPSERT_RESPONSIBILITY_ASSIGNMENT: &str = r#"
INSERT INTO responsibility_assignment (
    assignment_id, tenant_id, subject_id, target_kind, target_id, role, source,
    valid_from, valid_to, created_at, updated_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid, $4,
    NULLIF($5, '')::uuid, $6, $7, NULLIF($8, '')::timestamptz,
    NULLIF($9, '')::timestamptz, NULLIF($10, '')::timestamptz,
    NULLIF($11, '')::timestamptz
)
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
WHERE target_kind = $1 AND target_id = NULLIF($2, '')::uuid
ORDER BY created_at ASC, assignment_id ASC
LIMIT NULLIF($3, '')::int4 OFFSET NULLIF($4, '')::int4
"#;

    pub const UPSERT_INGEST_JOB: &str = r#"
INSERT INTO ingest_job (
    ingest_id, tenant_id, source_kind, source_name, received_at, status, payload_ref, error
) VALUES (
    $1, NULLIF($2, '')::uuid, $3, $4, NULLIF($5, '')::timestamptz, $6,
    NULLIF($7, ''), NULLIF($8, '')
)
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

    pub const UPSERT_HOST_RUNTIME_STATE: &str = r#"
INSERT INTO host_runtime_state (
    host_id, observed_at, boot_id, uptime_seconds, loadavg_1m, loadavg_5m, loadavg_15m,
    cpu_usage_pct, memory_used_bytes, memory_available_bytes, process_count, container_count,
    agent_health
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::timestamptz, NULLIF($3, ''),
    NULLIF($4, '')::bigint, NULLIF($5, '')::double precision,
    NULLIF($6, '')::double precision, NULLIF($7, '')::double precision,
    NULLIF($8, '')::double precision, NULLIF($9, '')::bigint,
    NULLIF($10, '')::bigint, NULLIF($11, '')::bigint,
    NULLIF($12, '')::bigint, $13
)
ON CONFLICT (host_id, observed_at) DO UPDATE SET
    boot_id = EXCLUDED.boot_id,
    uptime_seconds = EXCLUDED.uptime_seconds,
    loadavg_1m = EXCLUDED.loadavg_1m,
    loadavg_5m = EXCLUDED.loadavg_5m,
    loadavg_15m = EXCLUDED.loadavg_15m,
    cpu_usage_pct = EXCLUDED.cpu_usage_pct,
    memory_used_bytes = EXCLUDED.memory_used_bytes,
    memory_available_bytes = EXCLUDED.memory_available_bytes,
    process_count = EXCLUDED.process_count,
    container_count = EXCLUDED.container_count,
    agent_health = EXCLUDED.agent_health
"#;

    pub const LIST_HOST_RUNTIME_STATES: &str = r#"
SELECT host_id, observed_at, boot_id, uptime_seconds, loadavg_1m, loadavg_5m, loadavg_15m,
       cpu_usage_pct, memory_used_bytes, memory_available_bytes, process_count, container_count,
       agent_health
FROM host_runtime_state
WHERE host_id = NULLIF($1, '')::uuid
ORDER BY observed_at DESC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

    pub const UPSERT_PROCESS_RUNTIME_STATE: &str = r#"
INSERT INTO process_runtime_state (
    process_id, tenant_id, host_id, container_id, external_ref, pid, executable, command_line,
    process_state, memory_rss_kib, started_at, observed_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid,
    NULLIF($4, '')::uuid, NULLIF($5, ''), NULLIF($6, '')::integer, $7,
    NULLIF($8, ''), NULLIF($9, ''), NULLIF($10, '')::bigint,
    NULLIF($11, '')::timestamptz, NULLIF($12, '')::timestamptz
)
ON CONFLICT (process_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    host_id = EXCLUDED.host_id,
    container_id = EXCLUDED.container_id,
    external_ref = EXCLUDED.external_ref,
    pid = EXCLUDED.pid,
    executable = EXCLUDED.executable,
    command_line = EXCLUDED.command_line,
    process_state = EXCLUDED.process_state,
    memory_rss_kib = EXCLUDED.memory_rss_kib,
    started_at = EXCLUDED.started_at,
    observed_at = EXCLUDED.observed_at
"#;

    pub const LIST_PROCESS_RUNTIME_STATES: &str = r#"
SELECT process_id, tenant_id, host_id, container_id, external_ref, pid, executable, command_line,
       process_state, memory_rss_kib, started_at, observed_at
FROM process_runtime_state
WHERE host_id = NULLIF($1, '')::uuid
ORDER BY observed_at DESC, process_id ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

    pub const UPSERT_SERVICE_INSTANCE: &str = r#"
INSERT INTO service_instance (
    instance_id, tenant_id, service_id, workload_id, started_at, ended_at, last_seen_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid,
    NULLIF($4, '')::uuid, NULLIF($5, '')::timestamptz,
    NULLIF($6, '')::timestamptz, NULLIF($7, '')::timestamptz
)
ON CONFLICT (instance_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    service_id = EXCLUDED.service_id,
    workload_id = EXCLUDED.workload_id,
    started_at = EXCLUDED.started_at,
    ended_at = EXCLUDED.ended_at,
    last_seen_at = EXCLUDED.last_seen_at
"#;

    pub const GET_SERVICE_INSTANCE: &str = r#"
SELECT instance_id, tenant_id, service_id, workload_id, started_at, ended_at, last_seen_at
FROM service_instance
WHERE instance_id = NULLIF($1, '')::uuid
"#;

    pub const LIST_SERVICE_INSTANCES: &str = r#"
SELECT instance_id, tenant_id, service_id, workload_id, started_at, ended_at, last_seen_at
FROM service_instance
WHERE service_id = NULLIF($1, '')::uuid
ORDER BY started_at DESC, instance_id ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

    pub const UPSERT_RUNTIME_BINDING: &str = r#"
INSERT INTO runtime_binding (
    binding_id, instance_id, object_type, object_id, scope, confidence, source,
    valid_from, valid_to, created_at, updated_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, $3, NULLIF($4, '')::uuid, $5,
    $6, $7, NULLIF($8, '')::timestamptz, NULLIF($9, '')::timestamptz,
    NULLIF($10, '')::timestamptz, NULLIF($11, '')::timestamptz
)
ON CONFLICT (binding_id) DO UPDATE SET
    instance_id = EXCLUDED.instance_id,
    object_type = EXCLUDED.object_type,
    object_id = EXCLUDED.object_id,
    scope = EXCLUDED.scope,
    confidence = EXCLUDED.confidence,
    source = EXCLUDED.source,
    valid_from = EXCLUDED.valid_from,
    valid_to = EXCLUDED.valid_to,
    updated_at = EXCLUDED.updated_at
"#;

    pub const GET_RUNTIME_BINDING: &str = r#"
SELECT binding_id, instance_id, object_type, object_id, scope, confidence, source,
       valid_from, valid_to, created_at, updated_at
FROM runtime_binding
WHERE binding_id = NULLIF($1, '')::uuid
"#;

    pub const LIST_RUNTIME_BINDINGS_FOR_INSTANCE: &str = r#"
SELECT binding_id, instance_id, object_type, object_id, scope, confidence, source,
       valid_from, valid_to, created_at, updated_at
FROM runtime_binding
WHERE instance_id = NULLIF($1, '')::uuid
ORDER BY created_at ASC, binding_id ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

    pub const LIST_RUNTIME_BINDINGS_FOR_OBJECT: &str = r#"
SELECT binding_id, instance_id, object_type, object_id, scope, confidence, source,
       valid_from, valid_to, created_at, updated_at
FROM runtime_binding
WHERE object_type = $1 AND object_id = NULLIF($2, '')::uuid
ORDER BY created_at ASC, binding_id ASC
LIMIT NULLIF($3, '')::int4 OFFSET NULLIF($4, '')::int4
"#;
}

pub trait PostgresExecutor: Clone {
    fn exec(&self, sql: &str, params: &[String]) -> StorageResult<u64>;
    fn query_rows(&self, sql: &str, params: &[String]) -> StorageResult<Vec<Vec<String>>>;
    fn exec_batch(&self, sql: &str) -> StorageResult<()> {
        self.exec(sql, &[]).map(|_| ())
    }
    fn reset_public_schema(&self) -> StorageResult<()> {
        self.exec_batch(sql::RESET_PUBLIC_SCHEMA)
    }
}

#[derive(Debug, Clone)]
pub struct PostgresTopologyStore<E> {
    executor: E,
}

impl<E> PostgresTopologyStore<E> {
    pub fn new(executor: E) -> Self {
        Self { executor }
    }
}

impl<E> PostgresTopologyStore<E>
where
    E: PostgresExecutor,
{
    pub fn run_migrations(&self) -> StorageResult<()> {
        for migration in MIGRATIONS {
            self.executor.exec_batch(migration.sql)?;
        }
        Ok(())
    }

    pub fn reset_public_schema(&self) -> StorageResult<()> {
        self.executor.reset_public_schema()?;
        self.run_migrations()
    }
}

impl PostgresTopologyStore<LivePostgresExecutor> {
    pub async fn run_migrations_async(&self) -> StorageResult<()> {
        for migration in MIGRATIONS {
            self.executor.exec_batch_async(migration.sql).await?;
        }
        Ok(())
    }

    pub async fn reset_public_schema_async(&self) -> StorageResult<()> {
        self.executor
            .exec_batch_async(sql::RESET_PUBLIC_SCHEMA)
            .await?;
        self.run_migrations_async().await
    }
}

impl AsyncCatalogStore for PostgresTopologyStore<MemoryPostgresExecutor> {
    async fn upsert_business(
        &self,
        business: &topology_domain::BusinessDomain,
    ) -> StorageResult<()> {
        CatalogStore::upsert_business(self, business)
    }
    async fn get_business(
        &self,
        business_id: Uuid,
    ) -> StorageResult<Option<topology_domain::BusinessDomain>> {
        CatalogStore::get_business(self, business_id)
    }
    async fn list_businesses(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<topology_domain::BusinessDomain>> {
        CatalogStore::list_businesses(self, tenant_id, page)
    }
    async fn upsert_system(&self, system: &topology_domain::SystemBoundary) -> StorageResult<()> {
        CatalogStore::upsert_system(self, system)
    }
    async fn get_system(
        &self,
        system_id: Uuid,
    ) -> StorageResult<Option<topology_domain::SystemBoundary>> {
        CatalogStore::get_system(self, system_id)
    }
    async fn upsert_subsystem(&self, subsystem: &topology_domain::Subsystem) -> StorageResult<()> {
        CatalogStore::upsert_subsystem(self, subsystem)
    }
    async fn get_subsystem(
        &self,
        subsystem_id: Uuid,
    ) -> StorageResult<Option<topology_domain::Subsystem>> {
        CatalogStore::get_subsystem(self, subsystem_id)
    }
    async fn upsert_service(&self, service: &ServiceEntity) -> StorageResult<()> {
        CatalogStore::upsert_service(self, service)
    }
    async fn get_service(&self, service_id: Uuid) -> StorageResult<Option<ServiceEntity>> {
        CatalogStore::get_service(self, service_id)
    }
    async fn list_services(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<ServiceEntity>> {
        CatalogStore::list_services(self, tenant_id, page)
    }
    async fn upsert_cluster(
        &self,
        cluster: &topology_domain::ClusterInventory,
    ) -> StorageResult<()> {
        CatalogStore::upsert_cluster(self, cluster)
    }
    async fn get_cluster(
        &self,
        cluster_id: Uuid,
    ) -> StorageResult<Option<topology_domain::ClusterInventory>> {
        CatalogStore::get_cluster(self, cluster_id)
    }
    async fn upsert_namespace(
        &self,
        namespace: &topology_domain::NamespaceInventory,
    ) -> StorageResult<()> {
        CatalogStore::upsert_namespace(self, namespace)
    }
    async fn get_namespace(
        &self,
        namespace_id: Uuid,
    ) -> StorageResult<Option<topology_domain::NamespaceInventory>> {
        CatalogStore::get_namespace(self, namespace_id)
    }
    async fn upsert_workload(
        &self,
        workload: &topology_domain::WorkloadEntity,
    ) -> StorageResult<()> {
        CatalogStore::upsert_workload(self, workload)
    }
    async fn get_workload(
        &self,
        workload_id: Uuid,
    ) -> StorageResult<Option<topology_domain::WorkloadEntity>> {
        CatalogStore::get_workload(self, workload_id)
    }
    async fn upsert_pod(&self, pod: &topology_domain::PodInventory) -> StorageResult<()> {
        CatalogStore::upsert_pod(self, pod)
    }
    async fn get_pod(&self, pod_id: Uuid) -> StorageResult<Option<topology_domain::PodInventory>> {
        CatalogStore::get_pod(self, pod_id)
    }
    async fn upsert_host(&self, host: &HostInventory) -> StorageResult<()> {
        CatalogStore::upsert_host(self, host)
    }
    async fn get_host(&self, host_id: Uuid) -> StorageResult<Option<HostInventory>> {
        CatalogStore::get_host(self, host_id)
    }
    async fn list_hosts(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<HostInventory>> {
        CatalogStore::list_hosts(self, tenant_id, page)
    }
    async fn list_all_hosts(&self, page: Page) -> StorageResult<Vec<HostInventory>> {
        CatalogStore::list_all_hosts(self, page)
    }
    async fn upsert_network_domain(&self, domain: &NetworkDomain) -> StorageResult<()> {
        CatalogStore::upsert_network_domain(self, domain)
    }
    async fn get_network_domain(
        &self,
        network_domain_id: Uuid,
    ) -> StorageResult<Option<NetworkDomain>> {
        CatalogStore::get_network_domain(self, network_domain_id)
    }
    async fn upsert_network_segment(&self, segment: &NetworkSegment) -> StorageResult<()> {
        CatalogStore::upsert_network_segment(self, segment)
    }
    async fn get_network_segment(
        &self,
        network_segment_id: Uuid,
    ) -> StorageResult<Option<NetworkSegment>> {
        CatalogStore::get_network_segment(self, network_segment_id)
    }
    async fn list_network_segments(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<NetworkSegment>> {
        CatalogStore::list_network_segments(self, tenant_id, page)
    }
    async fn upsert_subject(&self, subject: &Subject) -> StorageResult<()> {
        CatalogStore::upsert_subject(self, subject)
    }
    async fn get_subject(&self, subject_id: Uuid) -> StorageResult<Option<Subject>> {
        CatalogStore::get_subject(self, subject_id)
    }
    async fn list_subjects(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<Subject>> {
        CatalogStore::list_subjects(self, tenant_id, page)
    }
}

impl AsyncRuntimeStore for PostgresTopologyStore<MemoryPostgresExecutor> {
    async fn insert_host_runtime_state(&self, state: &HostRuntimeState) -> StorageResult<()> {
        RuntimeStore::insert_host_runtime_state(self, state)
    }
    async fn list_host_runtime_states(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<HostRuntimeState>> {
        RuntimeStore::list_host_runtime_states(self, host_id, page)
    }
    async fn upsert_process_runtime_state(&self, state: &ProcessRuntimeState) -> StorageResult<()> {
        RuntimeStore::upsert_process_runtime_state(self, state)
    }
    async fn list_process_runtime_states(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ProcessRuntimeState>> {
        RuntimeStore::list_process_runtime_states(self, host_id, page)
    }
    async fn upsert_service_instance(&self, instance: &ServiceInstance) -> StorageResult<()> {
        RuntimeStore::upsert_service_instance(self, instance)
    }
    async fn get_service_instance(
        &self,
        instance_id: Uuid,
    ) -> StorageResult<Option<ServiceInstance>> {
        RuntimeStore::get_service_instance(self, instance_id)
    }
    async fn list_service_instances(
        &self,
        service_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ServiceInstance>> {
        RuntimeStore::list_service_instances(self, service_id, page)
    }
    async fn upsert_runtime_binding(&self, binding: &RuntimeBinding) -> StorageResult<()> {
        RuntimeStore::upsert_runtime_binding(self, binding)
    }
    async fn get_runtime_binding(&self, binding_id: Uuid) -> StorageResult<Option<RuntimeBinding>> {
        RuntimeStore::get_runtime_binding(self, binding_id)
    }
    async fn list_runtime_bindings_for_instance(
        &self,
        instance_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<RuntimeBinding>> {
        RuntimeStore::list_runtime_bindings_for_instance(self, instance_id, page)
    }
    async fn list_runtime_bindings_for_object(
        &self,
        object_type: RuntimeObjectType,
        object_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<RuntimeBinding>> {
        RuntimeStore::list_runtime_bindings_for_object(self, object_type, object_id, page)
    }
    async fn upsert_workload_pod_membership(
        &self,
        membership: &topology_domain::WorkloadPodMembership,
    ) -> StorageResult<()> {
        RuntimeStore::upsert_workload_pod_membership(self, membership)
    }
    async fn upsert_pod_placement(
        &self,
        placement: &topology_domain::PodPlacement,
    ) -> StorageResult<()> {
        RuntimeStore::upsert_pod_placement(self, placement)
    }
    async fn upsert_host_net_assoc(&self, assoc: &HostNetAssoc) -> StorageResult<()> {
        RuntimeStore::upsert_host_net_assoc(self, assoc)
    }
    async fn list_host_net_assocs(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<HostNetAssoc>> {
        RuntimeStore::list_host_net_assocs(self, host_id, page)
    }
}

impl AsyncGovernanceStore for PostgresTopologyStore<MemoryPostgresExecutor> {
    async fn upsert_responsibility_assignment(
        &self,
        assignment: &ResponsibilityAssignment,
    ) -> StorageResult<()> {
        GovernanceStore::upsert_responsibility_assignment(self, assignment)
    }
    async fn get_responsibility_assignment(
        &self,
        assignment_id: Uuid,
    ) -> StorageResult<Option<ResponsibilityAssignment>> {
        GovernanceStore::get_responsibility_assignment(self, assignment_id)
    }
    async fn list_responsibility_assignments_for_target(
        &self,
        target_kind: ObjectKind,
        target_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ResponsibilityAssignment>> {
        GovernanceStore::list_responsibility_assignments_for_target(
            self,
            target_kind,
            target_id,
            page,
        )
    }
}

impl AsyncIngestStore for PostgresTopologyStore<MemoryPostgresExecutor> {
    async fn record_ingest_job(&self, entry: IngestJobEntry) -> StorageResult<()> {
        IngestStore::record_ingest_job(self, entry)
    }

    async fn get_ingest_job(&self, ingest_id: &str) -> StorageResult<Option<IngestJobEntry>> {
        IngestStore::get_ingest_job(self, ingest_id)
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

    fn upsert_service(&self, service: &topology_domain::ServiceEntity) -> StorageResult<()> {
        self.executor.exec(
            sql::UPSERT_SERVICE,
            &[
                service.service_id.to_string(),
                service.tenant_id.0.to_string(),
                service
                    .business_id
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
                service
                    .system_id
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
                service
                    .subsystem_id
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
                service.name.clone(),
                service.namespace.clone().unwrap_or_default(),
                format!("{:?}", service.service_type),
                format!("{:?}", service.boundary),
                service.provider.clone().unwrap_or_default(),
                service.external_ref.clone().unwrap_or_default(),
                service.created_at.to_rfc3339(),
                service.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn get_service(
        &self,
        service_id: Uuid,
    ) -> StorageResult<Option<topology_domain::ServiceEntity>> {
        let rows = self
            .executor
            .query_rows(sql::GET_SERVICE, &[service_id.to_string()])?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_service(&row))
            .transpose()?)
    }

    fn list_services(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<topology_domain::ServiceEntity>> {
        let rows = self.executor.query_rows(
            sql::LIST_SERVICES,
            &[
                tenant_id.0.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter().map(|row| decode_service(&row)).collect()
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
        let sql = if host.machine_id.is_some() {
            sql::UPSERT_HOST
        } else {
            sql::UPSERT_HOST_WITHOUT_MACHINE_ID
        };
        self.executor.exec(
            sql,
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

    fn list_all_hosts(&self, page: Page) -> StorageResult<Vec<HostInventory>> {
        let rows = self.executor.query_rows(
            sql::LIST_ALL_HOSTS,
            &[page.limit.to_string(), page.offset.to_string()],
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
        state: &topology_domain::HostRuntimeState,
    ) -> StorageResult<()> {
        self.executor.exec(
            sql::UPSERT_HOST_RUNTIME_STATE,
            &[
                state.host_id.to_string(),
                state.observed_at.0.to_rfc3339(),
                state.boot_id.clone().unwrap_or_default(),
                state
                    .uptime_seconds
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                state.loadavg_1m.map(|v| v.to_string()).unwrap_or_default(),
                state.loadavg_5m.map(|v| v.to_string()).unwrap_or_default(),
                state.loadavg_15m.map(|v| v.to_string()).unwrap_or_default(),
                state
                    .cpu_usage_pct
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                state
                    .memory_used_bytes
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                state
                    .memory_available_bytes
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                state
                    .process_count
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                state
                    .container_count
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                format!("{:?}", state.agent_health),
            ],
        )?;
        Ok(())
    }

    fn list_host_runtime_states(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<topology_domain::HostRuntimeState>> {
        let rows = self.executor.query_rows(
            sql::LIST_HOST_RUNTIME_STATES,
            &[
                host_id.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter()
            .map(|row| decode_host_runtime_state(&row))
            .collect()
    }

    fn upsert_process_runtime_state(
        &self,
        state: &topology_domain::ProcessRuntimeState,
    ) -> StorageResult<()> {
        self.executor.exec(
            sql::UPSERT_PROCESS_RUNTIME_STATE,
            &[
                state.process_id.to_string(),
                state.tenant_id.0.to_string(),
                state.host_id.to_string(),
                state
                    .container_id
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                state.external_ref.clone().unwrap_or_default(),
                state.pid.to_string(),
                state.executable.clone(),
                state.command_line.clone().unwrap_or_default(),
                state.process_state.clone().unwrap_or_default(),
                state
                    .memory_rss_kib
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                state.started_at.to_rfc3339(),
                state.observed_at.0.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn list_process_runtime_states(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<topology_domain::ProcessRuntimeState>> {
        let rows = self.executor.query_rows(
            sql::LIST_PROCESS_RUNTIME_STATES,
            &[
                host_id.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter()
            .map(|row| decode_process_runtime_state(&row))
            .collect()
    }

    fn upsert_service_instance(
        &self,
        instance: &topology_domain::ServiceInstance,
    ) -> StorageResult<()> {
        self.executor.exec(
            sql::UPSERT_SERVICE_INSTANCE,
            &[
                instance.instance_id.to_string(),
                instance.tenant_id.0.to_string(),
                instance.service_id.to_string(),
                instance
                    .workload_id
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                instance.started_at.to_rfc3339(),
                instance
                    .ended_at
                    .map(|v| v.to_rfc3339())
                    .unwrap_or_default(),
                instance.last_seen_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn get_service_instance(
        &self,
        instance_id: Uuid,
    ) -> StorageResult<Option<topology_domain::ServiceInstance>> {
        let rows = self
            .executor
            .query_rows(sql::GET_SERVICE_INSTANCE, &[instance_id.to_string()])?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_service_instance(&row))
            .transpose()?)
    }

    fn list_service_instances(
        &self,
        service_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<topology_domain::ServiceInstance>> {
        let rows = self.executor.query_rows(
            sql::LIST_SERVICE_INSTANCES,
            &[
                service_id.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter()
            .map(|row| decode_service_instance(&row))
            .collect()
    }

    fn upsert_runtime_binding(
        &self,
        binding: &topology_domain::RuntimeBinding,
    ) -> StorageResult<()> {
        self.executor.exec(
            sql::UPSERT_RUNTIME_BINDING,
            &[
                binding.binding_id.to_string(),
                binding.instance_id.to_string(),
                format!("{:?}", binding.object_type),
                binding.object_id.to_string(),
                format!("{:?}", binding.scope),
                format!("{:?}", binding.confidence),
                binding.source.clone(),
                binding.validity.valid_from.to_rfc3339(),
                binding
                    .validity
                    .valid_to
                    .map(|v| v.to_rfc3339())
                    .unwrap_or_default(),
                binding.created_at.to_rfc3339(),
                binding.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn get_runtime_binding(
        &self,
        binding_id: Uuid,
    ) -> StorageResult<Option<topology_domain::RuntimeBinding>> {
        let rows = self
            .executor
            .query_rows(sql::GET_RUNTIME_BINDING, &[binding_id.to_string()])?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_runtime_binding(&row))
            .transpose()?)
    }

    fn list_runtime_bindings_for_instance(
        &self,
        instance_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<topology_domain::RuntimeBinding>> {
        let rows = self.executor.query_rows(
            sql::LIST_RUNTIME_BINDINGS_FOR_INSTANCE,
            &[
                instance_id.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter()
            .map(|row| decode_runtime_binding(&row))
            .collect()
    }

    fn list_runtime_bindings_for_object(
        &self,
        object_type: RuntimeObjectType,
        object_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<topology_domain::RuntimeBinding>> {
        let rows = self.executor.query_rows(
            sql::LIST_RUNTIME_BINDINGS_FOR_OBJECT,
            &[
                format!("{:?}", object_type),
                object_id.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter()
            .map(|row| decode_runtime_binding(&row))
            .collect()
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

#[derive(Clone)]
pub struct LivePostgresExecutor {
    client: Arc<tokio_postgres::Client>,
}

#[derive(Debug, Default)]
struct MemoryPostgresState {
    hosts: std::collections::BTreeMap<String, Vec<String>>,
    services: std::collections::BTreeMap<String, Vec<String>>,
    network_domains: std::collections::BTreeMap<String, Vec<String>>,
    network_segments: std::collections::BTreeMap<String, Vec<String>>,
    host_net_assocs: std::collections::BTreeMap<String, Vec<String>>,
    host_runtime_states: std::collections::BTreeMap<String, Vec<String>>,
    process_runtime_states: std::collections::BTreeMap<String, Vec<String>>,
    service_instances: std::collections::BTreeMap<String, Vec<String>>,
    runtime_bindings: std::collections::BTreeMap<String, Vec<String>>,
    subjects: std::collections::BTreeMap<String, Vec<String>>,
    responsibility_assignments: std::collections::BTreeMap<String, Vec<String>>,
    ingest_jobs: std::collections::BTreeMap<String, Vec<String>>,
}

impl PostgresExecutor for RecordingExecutor {
    fn exec(&self, sql: &str, params: &[String]) -> StorageResult<u64> {
        self.calls
            .lock()
            .map_err(|_| lock_failed())?
            .push((sql.to_string(), params.to_vec()));
        Ok(1)
    }

    fn query_rows(&self, sql: &str, params: &[String]) -> StorageResult<Vec<Vec<String>>> {
        self.calls
            .lock()
            .map_err(|_| lock_failed())?
            .push((sql.to_string(), params.to_vec()));
        Ok(Vec::new())
    }

    fn exec_batch(&self, sql: &str) -> StorageResult<()> {
        self.calls
            .lock()
            .map_err(|err| operation_failed(err.to_string()))?
            .push((sql.to_string(), Vec::new()));
        Ok(())
    }

    fn reset_public_schema(&self) -> StorageResult<()> {
        self.calls
            .lock()
            .map_err(|err| operation_failed(err.to_string()))?
            .push((sql::RESET_PUBLIC_SCHEMA.to_string(), Vec::new()));
        Ok(())
    }
}

impl PostgresExecutor for MemoryPostgresExecutor {
    fn exec(&self, sql: &str, params: &[String]) -> StorageResult<u64> {
        let mut state = self.state.lock().map_err(|_| lock_failed())?;

        match sql {
            value if value == sql::UPSERT_HOST => {
                let machine_id = params[4].clone();
                if !machine_id.is_empty() {
                    let existing_key = state
                        .hosts
                        .iter()
                        .find_map(|(key, row)| (row[4] == machine_id).then(|| key.clone()));
                    if let Some(existing_key) = existing_key {
                        state.hosts.insert(existing_key, params.to_vec());
                    } else {
                        state.hosts.insert(params[0].clone(), params.to_vec());
                    }
                } else {
                    state.hosts.insert(params[0].clone(), params.to_vec());
                }
            }
            value if value == sql::UPSERT_HOST_WITHOUT_MACHINE_ID => {
                state.hosts.insert(params[0].clone(), params.to_vec());
            }
            value if value == sql::UPSERT_SERVICE => {
                state.services.insert(params[0].clone(), params.to_vec());
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
            value if value == sql::UPSERT_HOST_RUNTIME_STATE => {
                state
                    .host_runtime_states
                    .insert(format!("{}:{}", params[0], params[1]), params.to_vec());
            }
            value if value == sql::UPSERT_PROCESS_RUNTIME_STATE => {
                state
                    .process_runtime_states
                    .insert(params[0].clone(), params.to_vec());
            }
            value if value == sql::UPSERT_SERVICE_INSTANCE => {
                state
                    .service_instances
                    .insert(params[0].clone(), params.to_vec());
            }
            value if value == sql::UPSERT_RUNTIME_BINDING => {
                state
                    .runtime_bindings
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
        let state = self.state.lock().map_err(|_| lock_failed())?;

        let rows = match sql {
            value if value == sql::GET_HOST => {
                state.hosts.get(&params[0]).into_iter().cloned().collect()
            }
            value if value == sql::GET_SERVICE => state
                .services
                .get(&params[0])
                .into_iter()
                .cloned()
                .collect(),
            value if value == sql::LIST_HOSTS => state
                .hosts
                .values()
                .filter(|row| row[1] == params[0])
                .cloned()
                .collect(),
            value if value == sql::LIST_SERVICES => state
                .services
                .values()
                .filter(|row| row[1] == params[0])
                .cloned()
                .collect(),
            value if value == sql::LIST_ALL_HOSTS => state.hosts.values().cloned().collect(),
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
            value if value == sql::LIST_HOST_RUNTIME_STATES => state
                .host_runtime_states
                .values()
                .filter(|row| row[0] == params[0])
                .cloned()
                .collect(),
            value if value == sql::LIST_PROCESS_RUNTIME_STATES => state
                .process_runtime_states
                .values()
                .filter(|row| row[2] == params[0])
                .cloned()
                .collect(),
            value if value == sql::GET_SERVICE_INSTANCE => state
                .service_instances
                .get(&params[0])
                .into_iter()
                .cloned()
                .collect(),
            value if value == sql::LIST_SERVICE_INSTANCES => state
                .service_instances
                .values()
                .filter(|row| row[2] == params[0])
                .cloned()
                .collect(),
            value if value == sql::GET_RUNTIME_BINDING => state
                .runtime_bindings
                .get(&params[0])
                .into_iter()
                .cloned()
                .collect(),
            value if value == sql::LIST_RUNTIME_BINDINGS_FOR_INSTANCE => state
                .runtime_bindings
                .values()
                .filter(|row| row[1] == params[0])
                .cloned()
                .collect(),
            value if value == sql::LIST_RUNTIME_BINDINGS_FOR_OBJECT => state
                .runtime_bindings
                .values()
                .filter(|row| row[2] == params[0] && row[3] == params[1])
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

    fn exec_batch(&self, _sql: &str) -> StorageResult<()> {
        Ok(())
    }

    fn reset_public_schema(&self) -> StorageResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|err| operation_failed(err.to_string()))?;
        *state = MemoryPostgresState::default();
        Ok(())
    }
}

impl LivePostgresExecutor {
    pub async fn new(connection_string: impl Into<String>) -> StorageResult<Self> {
        let connection_string = connection_string.into();
        let (client, connection) = tokio_postgres::connect(connection_string.as_str(), NoTls)
            .await
            .map_err(|err| operation_failed(format!("connect postgres: {err}")))?;

        tokio::spawn(async move {
            let _ = connection.await;
        });

        Ok(Self {
            client: Arc::new(client),
        })
    }

    async fn query_rows_async(
        &self,
        sql: &str,
        params: &[String],
    ) -> StorageResult<Vec<Vec<String>>> {
        let bind_params: Vec<&(dyn ToSql + Sync)> = params
            .iter()
            .map(|value| value as &(dyn ToSql + Sync))
            .collect();
        let rows = self
            .client
            .query(sql, &bind_params)
            .await
            .map_err(|err| operation_failed(format!("query postgres sql: {err}")))?;
        rows.into_iter().map(row_to_strings).collect()
    }

    async fn exec_async(&self, sql: &str, params: &[String]) -> StorageResult<u64> {
        let bind_params: Vec<&(dyn ToSql + Sync)> = params
            .iter()
            .map(|value| value as &(dyn ToSql + Sync))
            .collect();
        self.client
            .execute(sql, &bind_params)
            .await
            .map_err(|err| operation_failed(format!("execute postgres sql: {err}")))
    }

    async fn exec_batch_async(&self, sql: &str) -> StorageResult<()> {
        self.client
            .batch_execute(sql)
            .await
            .map_err(|err| operation_failed(format!("execute postgres batch sql: {err}")))
    }
}

impl AsyncCatalogStore for PostgresTopologyStore<LivePostgresExecutor> {
    async fn upsert_business(
        &self,
        _business: &topology_domain::BusinessDomain,
    ) -> StorageResult<()> {
        Err(not_configured())
    }

    async fn get_business(
        &self,
        _business_id: Uuid,
    ) -> StorageResult<Option<topology_domain::BusinessDomain>> {
        Ok(None)
    }

    async fn list_businesses(
        &self,
        _tenant_id: TenantId,
        _page: Page,
    ) -> StorageResult<Vec<topology_domain::BusinessDomain>> {
        Ok(Vec::new())
    }

    async fn upsert_system(&self, _system: &topology_domain::SystemBoundary) -> StorageResult<()> {
        Err(not_configured())
    }

    async fn get_system(
        &self,
        _system_id: Uuid,
    ) -> StorageResult<Option<topology_domain::SystemBoundary>> {
        Ok(None)
    }

    async fn upsert_subsystem(&self, _subsystem: &topology_domain::Subsystem) -> StorageResult<()> {
        Err(not_configured())
    }

    async fn get_subsystem(
        &self,
        _subsystem_id: Uuid,
    ) -> StorageResult<Option<topology_domain::Subsystem>> {
        Ok(None)
    }

    async fn upsert_service(&self, service: &ServiceEntity) -> StorageResult<()> {
        self.executor
            .exec_async(
                sql::UPSERT_SERVICE,
                &[
                    service.service_id.to_string(),
                    service.tenant_id.0.to_string(),
                    service
                        .business_id
                        .map(|id| id.to_string())
                        .unwrap_or_default(),
                    service
                        .system_id
                        .map(|id| id.to_string())
                        .unwrap_or_default(),
                    service
                        .subsystem_id
                        .map(|id| id.to_string())
                        .unwrap_or_default(),
                    service.name.clone(),
                    service.namespace.clone().unwrap_or_default(),
                    format!("{:?}", service.service_type),
                    format!("{:?}", service.boundary),
                    service.provider.clone().unwrap_or_default(),
                    service.external_ref.clone().unwrap_or_default(),
                    service.created_at.to_rfc3339(),
                    service.updated_at.to_rfc3339(),
                ],
            )
            .await?;
        Ok(())
    }

    async fn get_service(&self, service_id: Uuid) -> StorageResult<Option<ServiceEntity>> {
        let rows = self
            .executor
            .query_rows_async(sql::GET_SERVICE, &[service_id.to_string()])
            .await?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_service(&row))
            .transpose()?)
    }

    async fn upsert_host(&self, host: &HostInventory) -> StorageResult<()> {
        let sql = if host.machine_id.is_some() {
            sql::UPSERT_HOST
        } else {
            sql::UPSERT_HOST_WITHOUT_MACHINE_ID
        };
        self.executor
            .exec_async(
                sql,
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
            )
            .await?;
        Ok(())
    }

    async fn get_host(&self, host_id: Uuid) -> StorageResult<Option<HostInventory>> {
        let rows = self
            .executor
            .query_rows_async(sql::GET_HOST, &[host_id.to_string()])
            .await?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_host(&row))
            .transpose()?)
    }

    async fn list_hosts(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<HostInventory>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_HOSTS,
                &[
                    tenant_id.0.to_string(),
                    page.limit.to_string(),
                    page.offset.to_string(),
                ],
            )
            .await?;
        rows.into_iter().map(|row| decode_host(&row)).collect()
    }

    async fn list_all_hosts(&self, page: Page) -> StorageResult<Vec<HostInventory>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_ALL_HOSTS,
                &[page.limit.to_string(), page.offset.to_string()],
            )
            .await?;
        rows.into_iter().map(|row| decode_host(&row)).collect()
    }

    async fn upsert_network_domain(&self, domain: &NetworkDomain) -> StorageResult<()> {
        self.executor
            .exec_async(
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
            )
            .await?;
        Ok(())
    }

    async fn get_network_segment(
        &self,
        network_segment_id: Uuid,
    ) -> StorageResult<Option<NetworkSegment>> {
        let rows = self
            .executor
            .query_rows_async(sql::GET_NETWORK_SEGMENT, &[network_segment_id.to_string()])
            .await?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_network_segment(&row))
            .transpose()?)
    }

    async fn upsert_network_segment(&self, segment: &NetworkSegment) -> StorageResult<()> {
        self.executor
            .exec_async(
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
            )
            .await?;
        Ok(())
    }

    async fn list_network_segments(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<NetworkSegment>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_NETWORK_SEGMENTS,
                &[
                    tenant_id.0.to_string(),
                    page.limit.to_string(),
                    page.offset.to_string(),
                ],
            )
            .await?;
        rows.into_iter()
            .map(|row| decode_network_segment(&row))
            .collect()
    }

    async fn list_services(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<ServiceEntity>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_SERVICES,
                &[
                    tenant_id.0.to_string(),
                    page.limit.to_string(),
                    page.offset.to_string(),
                ],
            )
            .await?;
        rows.into_iter().map(|row| decode_service(&row)).collect()
    }

    async fn upsert_subject(&self, subject: &Subject) -> StorageResult<()> {
        self.executor
            .exec_async(
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
            )
            .await?;
        Ok(())
    }

    async fn get_subject(&self, subject_id: Uuid) -> StorageResult<Option<Subject>> {
        let rows = self
            .executor
            .query_rows_async(sql::GET_SUBJECT, &[subject_id.to_string()])
            .await?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_subject(&row))
            .transpose()?)
    }

    async fn list_subjects(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<Subject>> {
        let rows = self
            .executor
            .query_rows_async(
                "SELECT subject_id, tenant_id, subject_type, display_name, external_ref, email, is_active, created_at, updated_at FROM subject WHERE tenant_id = $1 ORDER BY display_name ASC, subject_id ASC LIMIT $2 OFFSET $3",
                &[
                    tenant_id.0.to_string(),
                    page.limit.to_string(),
                    page.offset.to_string(),
                ],
            )
            .await?;
        rows.into_iter().map(|row| decode_subject(&row)).collect()
    }

    async fn upsert_cluster(
        &self,
        _cluster: &topology_domain::ClusterInventory,
    ) -> StorageResult<()> {
        Err(not_configured())
    }

    async fn get_cluster(
        &self,
        _cluster_id: Uuid,
    ) -> StorageResult<Option<topology_domain::ClusterInventory>> {
        Ok(None)
    }

    async fn upsert_namespace(
        &self,
        _namespace: &topology_domain::NamespaceInventory,
    ) -> StorageResult<()> {
        Err(not_configured())
    }

    async fn get_namespace(
        &self,
        _namespace_id: Uuid,
    ) -> StorageResult<Option<topology_domain::NamespaceInventory>> {
        Ok(None)
    }

    async fn upsert_workload(
        &self,
        _workload: &topology_domain::WorkloadEntity,
    ) -> StorageResult<()> {
        Err(not_configured())
    }

    async fn get_workload(
        &self,
        _workload_id: Uuid,
    ) -> StorageResult<Option<topology_domain::WorkloadEntity>> {
        Ok(None)
    }

    async fn upsert_pod(&self, _pod: &topology_domain::PodInventory) -> StorageResult<()> {
        Err(not_configured())
    }

    async fn get_pod(&self, _pod_id: Uuid) -> StorageResult<Option<topology_domain::PodInventory>> {
        Ok(None)
    }

    async fn get_network_domain(
        &self,
        network_domain_id: Uuid,
    ) -> StorageResult<Option<NetworkDomain>> {
        let rows = self
            .executor
            .query_rows_async(
                "SELECT network_domain_id, tenant_id, environment_id, name, kind, description, created_at, updated_at FROM network_domain WHERE network_domain_id = $1",
                &[network_domain_id.to_string()],
            )
            .await?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_network_domain(&row))
            .transpose()?)
    }
}

impl AsyncRuntimeStore for PostgresTopologyStore<LivePostgresExecutor> {
    async fn insert_host_runtime_state(&self, state: &HostRuntimeState) -> StorageResult<()> {
        self.executor
            .exec_async(
                sql::UPSERT_HOST_RUNTIME_STATE,
                &[
                    state.host_id.to_string(),
                    state.observed_at.0.to_rfc3339(),
                    state.boot_id.clone().unwrap_or_default(),
                    state
                        .uptime_seconds
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    state.loadavg_1m.map(|v| v.to_string()).unwrap_or_default(),
                    state.loadavg_5m.map(|v| v.to_string()).unwrap_or_default(),
                    state.loadavg_15m.map(|v| v.to_string()).unwrap_or_default(),
                    state
                        .cpu_usage_pct
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    state
                        .memory_used_bytes
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    state
                        .memory_available_bytes
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    state
                        .process_count
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    state
                        .container_count
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    format!("{:?}", state.agent_health),
                ],
            )
            .await?;
        Ok(())
    }

    async fn list_host_runtime_states(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<HostRuntimeState>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_HOST_RUNTIME_STATES,
                &[
                    host_id.to_string(),
                    page.limit.to_string(),
                    page.offset.to_string(),
                ],
            )
            .await?;
        rows.into_iter()
            .map(|row| decode_host_runtime_state(&row))
            .collect()
    }

    async fn upsert_process_runtime_state(&self, state: &ProcessRuntimeState) -> StorageResult<()> {
        self.executor
            .exec_async(
                sql::UPSERT_PROCESS_RUNTIME_STATE,
                &[
                    state.process_id.to_string(),
                    state.tenant_id.0.to_string(),
                    state.host_id.to_string(),
                    state
                        .container_id
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    state.external_ref.clone().unwrap_or_default(),
                    state.pid.to_string(),
                    state.executable.clone(),
                    state.command_line.clone().unwrap_or_default(),
                    state.process_state.clone().unwrap_or_default(),
                    state
                        .memory_rss_kib
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    state.started_at.to_rfc3339(),
                    state.observed_at.0.to_rfc3339(),
                ],
            )
            .await?;
        Ok(())
    }

    async fn list_process_runtime_states(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ProcessRuntimeState>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_PROCESS_RUNTIME_STATES,
                &[
                    host_id.to_string(),
                    page.limit.to_string(),
                    page.offset.to_string(),
                ],
            )
            .await?;
        rows.into_iter()
            .map(|row| decode_process_runtime_state(&row))
            .collect()
    }

    async fn upsert_service_instance(&self, instance: &ServiceInstance) -> StorageResult<()> {
        self.executor
            .exec_async(
                sql::UPSERT_SERVICE_INSTANCE,
                &[
                    instance.instance_id.to_string(),
                    instance.tenant_id.0.to_string(),
                    instance.service_id.to_string(),
                    instance
                        .workload_id
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    instance.started_at.to_rfc3339(),
                    instance
                        .ended_at
                        .map(|v| v.to_rfc3339())
                        .unwrap_or_default(),
                    instance.last_seen_at.to_rfc3339(),
                ],
            )
            .await?;
        Ok(())
    }

    async fn get_service_instance(
        &self,
        instance_id: Uuid,
    ) -> StorageResult<Option<ServiceInstance>> {
        let rows = self
            .executor
            .query_rows_async(sql::GET_SERVICE_INSTANCE, &[instance_id.to_string()])
            .await?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_service_instance(&row))
            .transpose()?)
    }

    async fn list_service_instances(
        &self,
        service_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ServiceInstance>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_SERVICE_INSTANCES,
                &[
                    service_id.to_string(),
                    page.limit.to_string(),
                    page.offset.to_string(),
                ],
            )
            .await?;
        rows.into_iter()
            .map(|row| decode_service_instance(&row))
            .collect()
    }

    async fn upsert_runtime_binding(&self, binding: &RuntimeBinding) -> StorageResult<()> {
        self.executor
            .exec_async(
                sql::UPSERT_RUNTIME_BINDING,
                &[
                    binding.binding_id.to_string(),
                    binding.instance_id.to_string(),
                    format!("{:?}", binding.object_type),
                    binding.object_id.to_string(),
                    format!("{:?}", binding.scope),
                    format!("{:?}", binding.confidence),
                    binding.source.clone(),
                    binding.validity.valid_from.to_rfc3339(),
                    binding
                        .validity
                        .valid_to
                        .map(|v| v.to_rfc3339())
                        .unwrap_or_default(),
                    binding.created_at.to_rfc3339(),
                    binding.updated_at.to_rfc3339(),
                ],
            )
            .await?;
        Ok(())
    }

    async fn get_runtime_binding(&self, binding_id: Uuid) -> StorageResult<Option<RuntimeBinding>> {
        let rows = self
            .executor
            .query_rows_async(sql::GET_RUNTIME_BINDING, &[binding_id.to_string()])
            .await?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_runtime_binding(&row))
            .transpose()?)
    }

    async fn list_runtime_bindings_for_instance(
        &self,
        instance_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<RuntimeBinding>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_RUNTIME_BINDINGS_FOR_INSTANCE,
                &[
                    instance_id.to_string(),
                    page.limit.to_string(),
                    page.offset.to_string(),
                ],
            )
            .await?;
        rows.into_iter()
            .map(|row| decode_runtime_binding(&row))
            .collect()
    }

    async fn list_runtime_bindings_for_object(
        &self,
        object_type: RuntimeObjectType,
        object_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<RuntimeBinding>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_RUNTIME_BINDINGS_FOR_OBJECT,
                &[
                    format!("{:?}", object_type),
                    object_id.to_string(),
                    page.limit.to_string(),
                    page.offset.to_string(),
                ],
            )
            .await?;
        rows.into_iter()
            .map(|row| decode_runtime_binding(&row))
            .collect()
    }

    async fn list_host_net_assocs(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<HostNetAssoc>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_HOST_NET_ASSOCS,
                &[
                    host_id.to_string(),
                    page.limit.to_string(),
                    page.offset.to_string(),
                ],
            )
            .await?;
        rows.into_iter()
            .map(|row| decode_host_net_assoc(&row))
            .collect()
    }

    async fn upsert_host_net_assoc(&self, assoc: &HostNetAssoc) -> StorageResult<()> {
        self.executor
            .exec_async(
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
            )
            .await?;
        Ok(())
    }

    async fn upsert_workload_pod_membership(
        &self,
        _membership: &topology_domain::WorkloadPodMembership,
    ) -> StorageResult<()> {
        Err(not_configured())
    }

    async fn upsert_pod_placement(
        &self,
        _placement: &topology_domain::PodPlacement,
    ) -> StorageResult<()> {
        Err(not_configured())
    }
}

impl AsyncGovernanceStore for PostgresTopologyStore<LivePostgresExecutor> {
    async fn upsert_responsibility_assignment(
        &self,
        assignment: &ResponsibilityAssignment,
    ) -> StorageResult<()> {
        self.executor
            .exec_async(
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
            )
            .await?;
        Ok(())
    }

    async fn list_responsibility_assignments_for_target(
        &self,
        target_kind: ObjectKind,
        target_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ResponsibilityAssignment>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_RESPONSIBILITY_ASSIGNMENTS_FOR_TARGET,
                &[
                    format!("{:?}", target_kind),
                    target_id.to_string(),
                    page.limit.to_string(),
                    page.offset.to_string(),
                ],
            )
            .await?;
        rows.into_iter()
            .map(|row| decode_responsibility_assignment(&row))
            .collect()
    }

    async fn get_responsibility_assignment(
        &self,
        assignment_id: Uuid,
    ) -> StorageResult<Option<ResponsibilityAssignment>> {
        let rows = self
            .executor
            .query_rows_async(
                "SELECT assignment_id, tenant_id, subject_id, target_kind, target_id, role, source, valid_from, valid_to, created_at, updated_at FROM responsibility_assignment WHERE assignment_id = $1",
                &[assignment_id.to_string()],
            )
            .await?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_responsibility_assignment(&row))
            .transpose()?)
    }
}

impl AsyncIngestStore for PostgresTopologyStore<LivePostgresExecutor> {
    async fn record_ingest_job(&self, entry: IngestJobEntry) -> StorageResult<()> {
        self.executor
            .exec_async(
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
            )
            .await?;
        Ok(())
    }

    async fn get_ingest_job(&self, ingest_id: &str) -> StorageResult<Option<IngestJobEntry>> {
        let rows = self
            .executor
            .query_rows_async(sql::GET_INGEST_JOB, &[ingest_id.to_string()])
            .await?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_ingest_job(&row))
            .transpose()?)
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

fn decode_host_runtime_state(row: &[String]) -> StorageResult<HostRuntimeState> {
    Ok(HostRuntimeState {
        host_id: parse_uuid(&row[0])?,
        observed_at: topology_domain::ObservedAt(parse_datetime(&row[1])?),
        boot_id: empty_to_none(&row[2]),
        uptime_seconds: parse_optional_i64(&row[3])?,
        loadavg_1m: parse_optional_f64(&row[4])?,
        loadavg_5m: parse_optional_f64(&row[5])?,
        loadavg_15m: parse_optional_f64(&row[6])?,
        cpu_usage_pct: parse_optional_f64(&row[7])?,
        memory_used_bytes: parse_optional_i64(&row[8])?,
        memory_available_bytes: parse_optional_i64(&row[9])?,
        process_count: parse_optional_i64(&row[10])?,
        container_count: parse_optional_i64(&row[11])?,
        agent_health: parse_agent_health(&row[12])?,
    })
}

fn decode_process_runtime_state(row: &[String]) -> StorageResult<ProcessRuntimeState> {
    Ok(ProcessRuntimeState {
        process_id: parse_uuid(&row[0])?,
        tenant_id: TenantId(parse_uuid(&row[1])?),
        host_id: parse_uuid(&row[2])?,
        container_id: parse_optional_uuid(&row[3])?,
        external_ref: empty_to_none(&row[4]),
        pid: row[5]
            .parse::<i32>()
            .map_err(|err| operation_failed(err.to_string()))?,
        executable: row[6].clone(),
        command_line: empty_to_none(&row[7]),
        process_state: empty_to_none(&row[8]),
        memory_rss_kib: parse_optional_i64(&row[9])?,
        started_at: parse_datetime(&row[10])?,
        observed_at: topology_domain::ObservedAt(parse_datetime(&row[11])?),
    })
}

fn decode_service(row: &[String]) -> StorageResult<ServiceEntity> {
    Ok(ServiceEntity {
        service_id: parse_uuid(&row[0])?,
        tenant_id: TenantId(parse_uuid(&row[1])?),
        business_id: parse_optional_uuid(&row[2])?,
        system_id: parse_optional_uuid(&row[3])?,
        subsystem_id: parse_optional_uuid(&row[4])?,
        name: row[5].clone(),
        namespace: empty_to_none(&row[6]),
        service_type: parse_service_type(&row[7])?,
        boundary: parse_service_boundary(&row[8])?,
        provider: empty_to_none(&row[9]),
        external_ref: empty_to_none(&row[10]),
        created_at: parse_datetime(&row[11])?,
        updated_at: parse_datetime(&row[12])?,
    })
}

fn decode_service_instance(row: &[String]) -> StorageResult<ServiceInstance> {
    Ok(ServiceInstance {
        instance_id: parse_uuid(&row[0])?,
        tenant_id: TenantId(parse_uuid(&row[1])?),
        service_id: parse_uuid(&row[2])?,
        workload_id: parse_optional_uuid(&row[3])?,
        started_at: parse_datetime(&row[4])?,
        ended_at: empty_to_none(&row[5])
            .map(|value| parse_datetime(&value))
            .transpose()?,
        last_seen_at: parse_datetime(&row[6])?,
    })
}

fn decode_runtime_binding(row: &[String]) -> StorageResult<RuntimeBinding> {
    Ok(RuntimeBinding {
        binding_id: parse_uuid(&row[0])?,
        instance_id: parse_uuid(&row[1])?,
        object_type: parse_runtime_object_type(&row[2])?,
        object_id: parse_uuid(&row[3])?,
        scope: parse_binding_scope(&row[4])?,
        confidence: parse_confidence(&row[5])?,
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
    Uuid::parse_str(value).source_raw_err(StorageReason::DecodeFailed, "parse uuid")
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
        .source_raw_err(StorageReason::DecodeFailed, "parse rfc3339 datetime")
}

fn parse_optional_i64(value: &str) -> StorageResult<Option<i64>> {
    if value.is_empty() {
        Ok(None)
    } else {
        value
            .parse::<i64>()
            .map(Some)
            .map_err(|err| operation_failed(err.to_string()))
    }
}

fn parse_optional_f64(value: &str) -> StorageResult<Option<f64>> {
    if value.is_empty() {
        Ok(None)
    } else {
        value
            .parse::<f64>()
            .map(Some)
            .map_err(|err| operation_failed(err.to_string()))
    }
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
        other => Err(decode_failed(format!(
            "unsupported network domain kind: {other}"
        ))),
    }
}

fn parse_address_family(value: &str) -> StorageResult<topology_domain::AddressFamily> {
    match value {
        "Ipv4" => Ok(topology_domain::AddressFamily::Ipv4),
        "Ipv6" => Ok(topology_domain::AddressFamily::Ipv6),
        other => Err(decode_failed(format!(
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
        other => Err(decode_failed(format!("unsupported subject type: {other}"))),
    }
}

fn parse_service_type(value: &str) -> StorageResult<topology_domain::ServiceType> {
    match value {
        "Application" => Ok(topology_domain::ServiceType::Application),
        "Data" => Ok(topology_domain::ServiceType::Data),
        "Platform" => Ok(topology_domain::ServiceType::Platform),
        "Shared" => Ok(topology_domain::ServiceType::Shared),
        other => Err(operation_failed(format!(
            "unsupported service type: {other}"
        ))),
    }
}

fn parse_service_boundary(value: &str) -> StorageResult<topology_domain::ServiceBoundary> {
    match value {
        "Internal" => Ok(topology_domain::ServiceBoundary::Internal),
        "External" => Ok(topology_domain::ServiceBoundary::External),
        "Partner" => Ok(topology_domain::ServiceBoundary::Partner),
        "Saas" => Ok(topology_domain::ServiceBoundary::Saas),
        other => Err(operation_failed(format!(
            "unsupported service boundary: {other}"
        ))),
    }
}

fn parse_agent_health(value: &str) -> StorageResult<topology_domain::AgentHealth> {
    match value {
        "Healthy" => Ok(topology_domain::AgentHealth::Healthy),
        "Degraded" => Ok(topology_domain::AgentHealth::Degraded),
        "Protect" => Ok(topology_domain::AgentHealth::Protect),
        "Unavailable" => Ok(topology_domain::AgentHealth::Unavailable),
        other => Err(operation_failed(format!(
            "unsupported agent health: {other}"
        ))),
    }
}

fn parse_runtime_object_type(value: &str) -> StorageResult<RuntimeObjectType> {
    match value {
        "Process" => Ok(RuntimeObjectType::Process),
        "Container" => Ok(RuntimeObjectType::Container),
        "Pod" => Ok(RuntimeObjectType::Pod),
        other => Err(operation_failed(format!(
            "unsupported runtime object type: {other}"
        ))),
    }
}

fn parse_binding_scope(value: &str) -> StorageResult<topology_domain::BindingScope> {
    match value {
        "Declared" => Ok(topology_domain::BindingScope::Declared),
        "Observed" => Ok(topology_domain::BindingScope::Observed),
        "Inferred" => Ok(topology_domain::BindingScope::Inferred),
        other => Err(operation_failed(format!(
            "unsupported binding scope: {other}"
        ))),
    }
}

fn parse_confidence(value: &str) -> StorageResult<topology_domain::Confidence> {
    match value {
        "Low" => Ok(topology_domain::Confidence::Low),
        "Medium" => Ok(topology_domain::Confidence::Medium),
        "High" => Ok(topology_domain::Confidence::High),
        other => Err(operation_failed(format!("unsupported confidence: {other}"))),
    }
}

fn parse_object_kind(value: &str) -> StorageResult<ObjectKind> {
    match value {
        "Host" => Ok(ObjectKind::Host),
        "NetworkSegment" => Ok(ObjectKind::NetworkSegment),
        "Subject" => Ok(ObjectKind::Subject),
        other => Err(decode_failed(format!("unsupported object kind: {other}"))),
    }
}

fn parse_responsibility_role(value: &str) -> StorageResult<topology_domain::ResponsibilityRole> {
    match value {
        "Owner" => Ok(topology_domain::ResponsibilityRole::Owner),
        "Maintainer" => Ok(topology_domain::ResponsibilityRole::Maintainer),
        "Oncall" => Ok(topology_domain::ResponsibilityRole::Oncall),
        "Security" => Ok(topology_domain::ResponsibilityRole::Security),
        other => Err(decode_failed(format!(
            "unsupported responsibility role: {other}"
        ))),
    }
}

fn row_to_strings(row: tokio_postgres::Row) -> StorageResult<Vec<String>> {
    row.columns()
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let type_name = column.type_().name();
            let value = match type_name {
                "uuid" => row
                    .try_get::<usize, Option<Uuid>>(index)
                    .map(|value| value.map(|item| item.to_string()).unwrap_or_default()),
                "text" | "varchar" => row
                    .try_get::<usize, Option<String>>(index)
                    .map(|value| value.unwrap_or_default()),
                "timestamptz" => row
                    .try_get::<usize, Option<DateTime<Utc>>>(index)
                    .map(|value| value.map(|item| item.to_rfc3339()).unwrap_or_default()),
                "bool" => row
                    .try_get::<usize, Option<bool>>(index)
                    .map(|value| value.map(|item| item.to_string()).unwrap_or_default()),
                "int4" => row
                    .try_get::<usize, Option<i32>>(index)
                    .map(|value| value.map(|item| item.to_string()).unwrap_or_default()),
                "int8" => row
                    .try_get::<usize, Option<i64>>(index)
                    .map(|value| value.map(|item| item.to_string()).unwrap_or_default()),
                "float8" => row
                    .try_get::<usize, Option<f64>>(index)
                    .map(|value| value.map(|item| item.to_string()).unwrap_or_default()),
                other => {
                    return Err(operation_failed(format!(
                        "unsupported postgres column type: {other}"
                    )));
                }
            }
            .map_err(|err| operation_failed(err.to_string()))?;
            Ok(value)
        })
        .collect()
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
        let service = ServiceEntity {
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
        let host_runtime = HostRuntimeState {
            host_id: host.host_id,
            observed_at: topology_domain::ObservedAt(now),
            boot_id: Some("boot-1".to_string()),
            uptime_seconds: Some(42),
            loadavg_1m: Some(0.25),
            loadavg_5m: None,
            loadavg_15m: None,
            cpu_usage_pct: None,
            memory_used_bytes: Some(1024),
            memory_available_bytes: Some(2048),
            process_count: Some(3),
            container_count: Some(0),
            agent_health: topology_domain::AgentHealth::Healthy,
        };
        let process = ProcessRuntimeState {
            process_id: Uuid::new_v4(),
            tenant_id,
            host_id: host.host_id,
            container_id: None,
            external_ref: Some("hostname:node-01:pid:123:start:abc".to_string()),
            pid: 123,
            executable: "/usr/sbin/sshd".to_string(),
            command_line: Some("/usr/sbin/sshd -D".to_string()),
            process_state: Some("S".to_string()),
            memory_rss_kib: Some(7456),
            started_at: now,
            observed_at: topology_domain::ObservedAt(now),
        };
        let instance = ServiceInstance {
            instance_id: Uuid::new_v4(),
            tenant_id,
            service_id: Uuid::new_v4(),
            workload_id: None,
            started_at: now,
            ended_at: None,
            last_seen_at: now,
        };
        let binding = RuntimeBinding {
            binding_id: Uuid::new_v4(),
            instance_id: instance.instance_id,
            object_type: RuntimeObjectType::Process,
            object_id: process.process_id,
            scope: topology_domain::BindingScope::Observed,
            confidence: topology_domain::Confidence::Medium,
            source: "edge".to_string(),
            validity: topology_domain::ValidityWindow {
                valid_from: now,
                valid_to: None,
            },
            created_at: now,
            updated_at: now,
        };

        CatalogStore::upsert_host(&store, &host).unwrap();
        CatalogStore::upsert_service(&store, &service).unwrap();
        CatalogStore::upsert_network_domain(&store, &domain).unwrap();
        CatalogStore::upsert_network_segment(&store, &segment).unwrap();
        RuntimeStore::insert_host_runtime_state(&store, &host_runtime).unwrap();
        RuntimeStore::upsert_process_runtime_state(&store, &process).unwrap();
        RuntimeStore::upsert_service_instance(&store, &instance).unwrap();
        RuntimeStore::upsert_runtime_binding(&store, &binding).unwrap();
        CatalogStore::upsert_subject(&store, &subject).unwrap();
        GovernanceStore::upsert_responsibility_assignment(&store, &assignment).unwrap();
        IngestStore::record_ingest_job(
            &store,
            IngestJobEntry {
                ingest_id: "ing-1".to_string(),
                tenant_id,
                source_name: "demo".to_string(),
                source_kind: "batch_import".to_string(),
                received_at: now,
                status: "accepted".to_string(),
                payload_ref: None,
                error: None,
            },
        )
        .unwrap();

        assert!(
            CatalogStore::get_host(&store, host.host_id)
                .unwrap()
                .is_some()
        );
        assert_eq!(
            CatalogStore::list_hosts(&store, tenant_id, Page::default())
                .unwrap()
                .len(),
            1
        );
        assert!(
            CatalogStore::get_service(&store, service.service_id)
                .unwrap()
                .is_some()
        );
        assert_eq!(
            CatalogStore::list_services(&store, tenant_id, Page::default())
                .unwrap()
                .len(),
            1
        );
        assert!(
            CatalogStore::get_network_domain(&store, domain.network_domain_id)
                .unwrap()
                .is_some()
        );
        assert!(
            CatalogStore::get_network_segment(&store, segment.network_segment_id)
                .unwrap()
                .is_some()
        );
        assert_eq!(
            RuntimeStore::list_host_runtime_states(&store, host.host_id, Page::default())
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            RuntimeStore::list_process_runtime_states(&store, host.host_id, Page::default())
                .unwrap()
                .len(),
            1
        );
        assert!(
            RuntimeStore::get_service_instance(&store, instance.instance_id)
                .unwrap()
                .is_some()
        );
        assert!(
            RuntimeStore::get_runtime_binding(&store, binding.binding_id)
                .unwrap()
                .is_some()
        );
        assert!(
            CatalogStore::get_subject(&store, subject.subject_id)
                .unwrap()
                .is_some()
        );
        assert_eq!(
            GovernanceStore::list_responsibility_assignments_for_target(
                &store,
                ObjectKind::Host,
                host.host_id,
                Page::default(),
            )
            .unwrap()
            .len(),
            1
        );
        assert!(
            IngestStore::get_ingest_job(&store, "ing-1")
                .unwrap()
                .is_some()
        );
    }
}
