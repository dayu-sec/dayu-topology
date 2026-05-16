use topology_domain::{
    HostInventory, HostNetAssoc, HostRuntimeState, NetworkDomain, NetworkSegment,
    ProcessRuntimeState, ResponsibilityAssignment, RuntimeBinding, ServiceEntity, ServiceInstance,
    Subject, TenantId,
};
use uuid::Uuid;

use crate::{
    AsyncCatalogStore, AsyncGovernanceStore, AsyncRuntimeStore, CatalogStore, GovernanceStore,
    Page, RuntimeStore, StorageResult,
};

use super::{AsyncIngestStore, InMemoryTopologyStore, IngestJobEntry, IngestStore};

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
