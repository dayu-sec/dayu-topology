use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use topology_domain::{
    HostInventory, HostNetAssoc, HostRuntimeState, NetworkDomain, NetworkSegment,
    ProcessRuntimeState, ResponsibilityAssignment, RuntimeBinding, ServiceEntity, ServiceInstance,
    Subject, TenantId,
};
use uuid::Uuid;

use crate::{
    AsyncCatalogStore, AsyncGovernanceStore, AsyncRuntimeStore, CatalogStore, GovernanceStore,
    Page, RuntimeStore, StorageResult, not_configured, operation_failed,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestJobEntry {
    pub ingest_id: String,
    pub tenant_id: TenantId,
    pub source_name: String,
    pub source_kind: String,
    pub received_at: DateTime<Utc>,
    pub status: String,
    pub payload_ref: Option<String>,
    pub error: Option<String>,
}

pub trait IngestStore {
    fn record_ingest_job(&self, entry: IngestJobEntry) -> StorageResult<()>;
    fn get_ingest_job(&self, ingest_id: &str) -> StorageResult<Option<IngestJobEntry>>;
}

#[allow(async_fn_in_trait)]
pub trait AsyncIngestStore: Sync {
    async fn record_ingest_job(&self, entry: IngestJobEntry) -> StorageResult<()>;
    async fn get_ingest_job(&self, ingest_id: &str) -> StorageResult<Option<IngestJobEntry>>;
}

#[derive(Debug, Default)]
struct MemoryState {
    hosts: Vec<HostInventory>,
    network_domains: Vec<NetworkDomain>,
    network_segments: Vec<NetworkSegment>,
    host_net_assocs: Vec<HostNetAssoc>,
    host_runtime_states: Vec<HostRuntimeState>,
    process_runtime_states: Vec<ProcessRuntimeState>,
    services: Vec<ServiceEntity>,
    service_instances: Vec<ServiceInstance>,
    runtime_bindings: Vec<RuntimeBinding>,
    subjects: Vec<Subject>,
    responsibility_assignments: Vec<ResponsibilityAssignment>,
    ingest_jobs: Vec<IngestJobEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryTopologyStore {
    state: Arc<Mutex<MemoryState>>,
}

impl InMemoryTopologyStore {
    fn with_state<T>(&self, f: impl FnOnce(&mut MemoryState) -> T) -> StorageResult<T> {
        let mut state = self
            .state
            .lock()
            .map_err(|err| operation_failed(err.to_string()))?;
        Ok(f(&mut state))
    }
}

impl AsyncCatalogStore for InMemoryTopologyStore {
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

impl AsyncRuntimeStore for InMemoryTopologyStore {
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
        object_type: topology_domain::RuntimeObjectType,
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

impl AsyncGovernanceStore for InMemoryTopologyStore {
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
        target_kind: topology_domain::ObjectKind,
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

impl AsyncIngestStore for InMemoryTopologyStore {
    async fn record_ingest_job(&self, entry: IngestJobEntry) -> StorageResult<()> {
        IngestStore::record_ingest_job(self, entry)
    }

    async fn get_ingest_job(&self, ingest_id: &str) -> StorageResult<Option<IngestJobEntry>> {
        IngestStore::get_ingest_job(self, ingest_id)
    }
}

impl CatalogStore for InMemoryTopologyStore {
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
        self.with_state(|state| {
            if let Some(existing) = state
                .services
                .iter_mut()
                .find(|item| item.service_id == service.service_id)
            {
                *existing = service.clone();
            } else {
                state.services.push(service.clone());
            }
        })
    }

    fn get_service(
        &self,
        service_id: Uuid,
    ) -> StorageResult<Option<topology_domain::ServiceEntity>> {
        self.with_state(|state| {
            state
                .services
                .iter()
                .find(|item| item.service_id == service_id)
                .cloned()
        })
    }

    fn list_services(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<topology_domain::ServiceEntity>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .services
                .iter()
                .filter(|item| item.tenant_id == tenant_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
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
        self.with_state(|state| {
            if let Some(existing) = state
                .hosts
                .iter_mut()
                .find(|item| item.host_id == host.host_id)
            {
                *existing = host.clone();
            } else {
                state.hosts.push(host.clone());
            }
        })
    }

    fn get_host(&self, host_id: Uuid) -> StorageResult<Option<HostInventory>> {
        self.with_state(|state| {
            state
                .hosts
                .iter()
                .find(|item| item.host_id == host_id)
                .cloned()
        })
    }

    fn list_hosts(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<HostInventory>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .hosts
                .iter()
                .filter(|item| item.tenant_id == tenant_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }

    fn list_all_hosts(&self, page: Page) -> StorageResult<Vec<HostInventory>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            let mut hosts = state.hosts.clone();
            hosts.sort_by(|left, right| {
                left.host_name
                    .cmp(&right.host_name)
                    .then(left.host_id.cmp(&right.host_id))
            });
            hosts
                .into_iter()
                .skip(start)
                .take(page.limit as usize)
                .collect()
        })
    }

    fn upsert_network_domain(&self, domain: &NetworkDomain) -> StorageResult<()> {
        self.with_state(|state| {
            if let Some(existing) = state
                .network_domains
                .iter_mut()
                .find(|item| item.network_domain_id == domain.network_domain_id)
            {
                *existing = domain.clone();
            } else {
                state.network_domains.push(domain.clone());
            }
        })
    }

    fn get_network_domain(&self, network_domain_id: Uuid) -> StorageResult<Option<NetworkDomain>> {
        self.with_state(|state| {
            state
                .network_domains
                .iter()
                .find(|item| item.network_domain_id == network_domain_id)
                .cloned()
        })
    }

    fn upsert_network_segment(&self, segment: &NetworkSegment) -> StorageResult<()> {
        self.with_state(|state| {
            if let Some(existing) = state
                .network_segments
                .iter_mut()
                .find(|item| item.network_segment_id == segment.network_segment_id)
            {
                *existing = segment.clone();
            } else {
                state.network_segments.push(segment.clone());
            }
        })
    }

    fn get_network_segment(
        &self,
        network_segment_id: Uuid,
    ) -> StorageResult<Option<NetworkSegment>> {
        self.with_state(|state| {
            state
                .network_segments
                .iter()
                .find(|item| item.network_segment_id == network_segment_id)
                .cloned()
        })
    }

    fn list_network_segments(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<NetworkSegment>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .network_segments
                .iter()
                .filter(|item| item.tenant_id == tenant_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }

    fn upsert_subject(&self, subject: &Subject) -> StorageResult<()> {
        self.with_state(|state| {
            if let Some(existing) = state
                .subjects
                .iter_mut()
                .find(|item| item.subject_id == subject.subject_id)
            {
                *existing = subject.clone();
            } else {
                state.subjects.push(subject.clone());
            }
        })
    }

    fn get_subject(&self, subject_id: Uuid) -> StorageResult<Option<Subject>> {
        self.with_state(|state| {
            state
                .subjects
                .iter()
                .find(|item| item.subject_id == subject_id)
                .cloned()
        })
    }

    fn list_subjects(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<Subject>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .subjects
                .iter()
                .filter(|item| item.tenant_id == tenant_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }
}

impl RuntimeStore for InMemoryTopologyStore {
    fn insert_host_runtime_state(
        &self,
        state: &topology_domain::HostRuntimeState,
    ) -> StorageResult<()> {
        self.with_state(|store| {
            if let Some(existing) = store
                .host_runtime_states
                .iter_mut()
                .find(|item| item.host_id == state.host_id && item.observed_at == state.observed_at)
            {
                *existing = state.clone();
            } else {
                store.host_runtime_states.push(state.clone());
            }
        })
    }

    fn list_host_runtime_states(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<topology_domain::HostRuntimeState>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .host_runtime_states
                .iter()
                .filter(|item| item.host_id == host_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }

    fn upsert_process_runtime_state(
        &self,
        state: &topology_domain::ProcessRuntimeState,
    ) -> StorageResult<()> {
        self.with_state(|store| {
            if let Some(existing) = store
                .process_runtime_states
                .iter_mut()
                .find(|item| item.process_id == state.process_id)
            {
                *existing = state.clone();
            } else {
                store.process_runtime_states.push(state.clone());
            }
        })
    }

    fn list_process_runtime_states(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<topology_domain::ProcessRuntimeState>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .process_runtime_states
                .iter()
                .filter(|item| item.host_id == host_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }

    fn upsert_service_instance(
        &self,
        instance: &topology_domain::ServiceInstance,
    ) -> StorageResult<()> {
        self.with_state(|state| {
            if let Some(existing) = state
                .service_instances
                .iter_mut()
                .find(|item| item.instance_id == instance.instance_id)
            {
                *existing = instance.clone();
            } else {
                state.service_instances.push(instance.clone());
            }
        })
    }

    fn get_service_instance(
        &self,
        instance_id: Uuid,
    ) -> StorageResult<Option<topology_domain::ServiceInstance>> {
        self.with_state(|state| {
            state
                .service_instances
                .iter()
                .find(|item| item.instance_id == instance_id)
                .cloned()
        })
    }

    fn list_service_instances(
        &self,
        service_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<topology_domain::ServiceInstance>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .service_instances
                .iter()
                .filter(|item| item.service_id == service_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }

    fn upsert_runtime_binding(
        &self,
        binding: &topology_domain::RuntimeBinding,
    ) -> StorageResult<()> {
        self.with_state(|state| {
            if let Some(existing) = state
                .runtime_bindings
                .iter_mut()
                .find(|item| item.binding_id == binding.binding_id)
            {
                *existing = binding.clone();
            } else {
                state.runtime_bindings.push(binding.clone());
            }
        })
    }

    fn get_runtime_binding(
        &self,
        binding_id: Uuid,
    ) -> StorageResult<Option<topology_domain::RuntimeBinding>> {
        self.with_state(|state| {
            state
                .runtime_bindings
                .iter()
                .find(|item| item.binding_id == binding_id)
                .cloned()
        })
    }

    fn list_runtime_bindings_for_instance(
        &self,
        instance_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<topology_domain::RuntimeBinding>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .runtime_bindings
                .iter()
                .filter(|item| item.instance_id == instance_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }

    fn list_runtime_bindings_for_object(
        &self,
        object_type: topology_domain::RuntimeObjectType,
        object_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<topology_domain::RuntimeBinding>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .runtime_bindings
                .iter()
                .filter(|item| item.object_type == object_type && item.object_id == object_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
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
        self.with_state(|state| {
            if let Some(existing) = state
                .host_net_assocs
                .iter_mut()
                .find(|item| item.assoc_id == assoc.assoc_id)
            {
                *existing = assoc.clone();
            } else {
                state.host_net_assocs.push(assoc.clone());
            }
        })
    }

    fn list_host_net_assocs(&self, host_id: Uuid, page: Page) -> StorageResult<Vec<HostNetAssoc>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .host_net_assocs
                .iter()
                .filter(|item| item.host_id == host_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }
}

impl GovernanceStore for InMemoryTopologyStore {
    fn upsert_responsibility_assignment(
        &self,
        assignment: &ResponsibilityAssignment,
    ) -> StorageResult<()> {
        self.with_state(|state| {
            if let Some(existing) = state
                .responsibility_assignments
                .iter_mut()
                .find(|item| item.assignment_id == assignment.assignment_id)
            {
                *existing = assignment.clone();
            } else {
                state.responsibility_assignments.push(assignment.clone());
            }
        })
    }

    fn get_responsibility_assignment(
        &self,
        assignment_id: Uuid,
    ) -> StorageResult<Option<ResponsibilityAssignment>> {
        self.with_state(|state| {
            state
                .responsibility_assignments
                .iter()
                .find(|item| item.assignment_id == assignment_id)
                .cloned()
        })
    }

    fn list_responsibility_assignments_for_target(
        &self,
        target_kind: topology_domain::ObjectKind,
        target_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ResponsibilityAssignment>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .responsibility_assignments
                .iter()
                .filter(|item| item.target_kind == target_kind && item.target_id == target_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }
}

impl IngestStore for InMemoryTopologyStore {
    fn record_ingest_job(&self, entry: IngestJobEntry) -> StorageResult<()> {
        self.with_state(|state| {
            if let Some(existing) = state
                .ingest_jobs
                .iter_mut()
                .find(|item| item.ingest_id == entry.ingest_id)
            {
                *existing = entry;
            } else {
                state.ingest_jobs.push(entry);
            }
        })
    }

    fn get_ingest_job(&self, ingest_id: &str) -> StorageResult<Option<IngestJobEntry>> {
        self.with_state(|state| {
            state
                .ingest_jobs
                .iter()
                .find(|item| item.ingest_id == ingest_id)
                .cloned()
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use topology_domain::{AddressFamily, EnvironmentId, NetworkDomainKind};

    use super::*;

    #[test]
    fn in_memory_store_persists_host_network_and_ingest_job() {
        let store = InMemoryTopologyStore::default();
        let tenant_id = TenantId(Uuid::new_v4());
        let host = HostInventory {
            host_id: Uuid::new_v4(),
            tenant_id,
            environment_id: Some(EnvironmentId(Uuid::new_v4())),
            host_name: "node-01".to_string(),
            machine_id: Some("machine-01".to_string()),
            os_name: Some("linux".to_string()),
            os_version: Some("6.8".to_string()),
            created_at: Utc::now(),
            last_inventory_at: Utc::now(),
        };
        let domain = NetworkDomain {
            network_domain_id: Uuid::new_v4(),
            tenant_id,
            environment_id: None,
            name: "default".to_string(),
            kind: NetworkDomainKind::Unknown,
            description: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let segment = NetworkSegment {
            network_segment_id: Uuid::new_v4(),
            tenant_id,
            network_domain_id: Some(domain.network_domain_id),
            environment_id: None,
            name: "office".to_string(),
            cidr: Some("192.168.0.0/24".to_string()),
            gateway_ip: Some("192.168.0.1".to_string()),
            address_family: AddressFamily::Ipv4,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        CatalogStore::upsert_host(&store, &host).unwrap();
        CatalogStore::upsert_network_domain(&store, &domain).unwrap();
        CatalogStore::upsert_network_segment(&store, &segment).unwrap();
        IngestStore::record_ingest_job(
            &store,
            IngestJobEntry {
                ingest_id: "ing-1".to_string(),
                tenant_id,
                source_name: "fixture".to_string(),
                source_kind: "batch_import".to_string(),
                received_at: Utc::now(),
                status: "accepted".to_string(),
                payload_ref: None,
                error: None,
            },
        )
        .unwrap();

        assert_eq!(
            CatalogStore::list_hosts(&store, tenant_id, Page::default())
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            CatalogStore::list_network_segments(&store, tenant_id, Page::default())
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
