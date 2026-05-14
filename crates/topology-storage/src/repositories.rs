use topology_domain::{
    BusinessDomain, ClusterInventory, ExternalIdentityLink, ExternalSyncCursor, HostInventory,
    HostNetAssoc, HostRuntimeState, NamespaceInventory, NetworkDomain, NetworkSegment, ObjectKind,
    PodInventory, PodPlacement, ProcessRuntimeState, ResponsibilityAssignment, RuntimeBinding,
    ServiceEntity, ServiceInstance, Subject, Subsystem, SystemBoundary, TenantId, WorkloadEntity,
    WorkloadPodMembership,
};
use uuid::Uuid;

use crate::StorageResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page {
    pub limit: u32,
    pub offset: u32,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            limit: 100,
            offset: 0,
        }
    }
}

pub trait CatalogStore {
    fn upsert_business(&self, business: &BusinessDomain) -> StorageResult<()>;
    fn get_business(&self, business_id: Uuid) -> StorageResult<Option<BusinessDomain>>;
    fn list_businesses(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<BusinessDomain>>;

    fn upsert_system(&self, system: &SystemBoundary) -> StorageResult<()>;
    fn get_system(&self, system_id: Uuid) -> StorageResult<Option<SystemBoundary>>;

    fn upsert_subsystem(&self, subsystem: &Subsystem) -> StorageResult<()>;
    fn get_subsystem(&self, subsystem_id: Uuid) -> StorageResult<Option<Subsystem>>;

    fn upsert_service(&self, service: &ServiceEntity) -> StorageResult<()>;
    fn get_service(&self, service_id: Uuid) -> StorageResult<Option<ServiceEntity>>;
    fn list_services(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<ServiceEntity>>;

    fn upsert_cluster(&self, cluster: &ClusterInventory) -> StorageResult<()>;
    fn get_cluster(&self, cluster_id: Uuid) -> StorageResult<Option<ClusterInventory>>;

    fn upsert_namespace(&self, namespace: &NamespaceInventory) -> StorageResult<()>;
    fn get_namespace(&self, namespace_id: Uuid) -> StorageResult<Option<NamespaceInventory>>;

    fn upsert_workload(&self, workload: &WorkloadEntity) -> StorageResult<()>;
    fn get_workload(&self, workload_id: Uuid) -> StorageResult<Option<WorkloadEntity>>;

    fn upsert_pod(&self, pod: &PodInventory) -> StorageResult<()>;
    fn get_pod(&self, pod_id: Uuid) -> StorageResult<Option<PodInventory>>;

    fn upsert_host(&self, host: &HostInventory) -> StorageResult<()>;
    fn get_host(&self, host_id: Uuid) -> StorageResult<Option<HostInventory>>;
    fn list_hosts(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<HostInventory>>;
    fn list_all_hosts(&self, page: Page) -> StorageResult<Vec<HostInventory>>;

    fn upsert_network_domain(&self, domain: &NetworkDomain) -> StorageResult<()>;
    fn get_network_domain(&self, network_domain_id: Uuid) -> StorageResult<Option<NetworkDomain>>;

    fn upsert_network_segment(&self, segment: &NetworkSegment) -> StorageResult<()>;
    fn get_network_segment(
        &self,
        network_segment_id: Uuid,
    ) -> StorageResult<Option<NetworkSegment>>;
    fn list_network_segments(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<NetworkSegment>>;

    fn upsert_subject(&self, subject: &Subject) -> StorageResult<()>;
    fn get_subject(&self, subject_id: Uuid) -> StorageResult<Option<Subject>>;
    fn list_subjects(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<Subject>>;
}

// Temporary async shim over the sync storage traits.
// This lets upper layers migrate to async signatures before the concrete stores
// are rewritten to be natively async.
#[allow(async_fn_in_trait)]
pub trait AsyncCatalogStore: CatalogStore + Sync {
    async fn upsert_business(&self, business: &BusinessDomain) -> StorageResult<()> {
        CatalogStore::upsert_business(self, business)
    }
    async fn get_business(&self, business_id: Uuid) -> StorageResult<Option<BusinessDomain>> {
        CatalogStore::get_business(self, business_id)
    }
    async fn list_businesses(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<BusinessDomain>> {
        CatalogStore::list_businesses(self, tenant_id, page)
    }
    async fn upsert_system(&self, system: &SystemBoundary) -> StorageResult<()> {
        CatalogStore::upsert_system(self, system)
    }
    async fn get_system(&self, system_id: Uuid) -> StorageResult<Option<SystemBoundary>> {
        CatalogStore::get_system(self, system_id)
    }
    async fn upsert_subsystem(&self, subsystem: &Subsystem) -> StorageResult<()> {
        CatalogStore::upsert_subsystem(self, subsystem)
    }
    async fn get_subsystem(&self, subsystem_id: Uuid) -> StorageResult<Option<Subsystem>> {
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
    async fn upsert_cluster(&self, cluster: &ClusterInventory) -> StorageResult<()> {
        CatalogStore::upsert_cluster(self, cluster)
    }
    async fn get_cluster(&self, cluster_id: Uuid) -> StorageResult<Option<ClusterInventory>> {
        CatalogStore::get_cluster(self, cluster_id)
    }
    async fn upsert_namespace(&self, namespace: &NamespaceInventory) -> StorageResult<()> {
        CatalogStore::upsert_namespace(self, namespace)
    }
    async fn get_namespace(&self, namespace_id: Uuid) -> StorageResult<Option<NamespaceInventory>> {
        CatalogStore::get_namespace(self, namespace_id)
    }
    async fn upsert_workload(&self, workload: &WorkloadEntity) -> StorageResult<()> {
        CatalogStore::upsert_workload(self, workload)
    }
    async fn get_workload(&self, workload_id: Uuid) -> StorageResult<Option<WorkloadEntity>> {
        CatalogStore::get_workload(self, workload_id)
    }
    async fn upsert_pod(&self, pod: &PodInventory) -> StorageResult<()> {
        CatalogStore::upsert_pod(self, pod)
    }
    async fn get_pod(&self, pod_id: Uuid) -> StorageResult<Option<PodInventory>> {
        CatalogStore::get_pod(self, pod_id)
    }
    async fn upsert_host(&self, host: &HostInventory) -> StorageResult<()> {
        CatalogStore::upsert_host(self, host)
    }
    async fn get_host(&self, host_id: Uuid) -> StorageResult<Option<HostInventory>> {
        CatalogStore::get_host(self, host_id)
    }
    async fn list_hosts(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<HostInventory>> {
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

pub trait RuntimeStore {
    fn insert_host_runtime_state(&self, state: &HostRuntimeState) -> StorageResult<()>;
    fn list_host_runtime_states(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<HostRuntimeState>>;

    fn upsert_process_runtime_state(&self, state: &ProcessRuntimeState) -> StorageResult<()>;
    fn list_process_runtime_states(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ProcessRuntimeState>>;

    fn upsert_service_instance(&self, instance: &ServiceInstance) -> StorageResult<()>;
    fn get_service_instance(&self, instance_id: Uuid) -> StorageResult<Option<ServiceInstance>>;

    fn upsert_runtime_binding(&self, binding: &RuntimeBinding) -> StorageResult<()>;
    fn get_runtime_binding(&self, binding_id: Uuid) -> StorageResult<Option<RuntimeBinding>>;
    fn list_runtime_bindings_for_instance(
        &self,
        instance_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<RuntimeBinding>>;

    fn upsert_workload_pod_membership(
        &self,
        membership: &WorkloadPodMembership,
    ) -> StorageResult<()>;

    fn upsert_pod_placement(&self, placement: &PodPlacement) -> StorageResult<()>;

    fn upsert_host_net_assoc(&self, assoc: &HostNetAssoc) -> StorageResult<()>;
    fn list_host_net_assocs(&self, host_id: Uuid, page: Page) -> StorageResult<Vec<HostNetAssoc>>;
}

#[allow(async_fn_in_trait)]
pub trait AsyncRuntimeStore: RuntimeStore + Sync {
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
    async fn get_service_instance(&self, instance_id: Uuid) -> StorageResult<Option<ServiceInstance>> {
        RuntimeStore::get_service_instance(self, instance_id)
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
    async fn upsert_workload_pod_membership(
        &self,
        membership: &WorkloadPodMembership,
    ) -> StorageResult<()> {
        RuntimeStore::upsert_workload_pod_membership(self, membership)
    }
    async fn upsert_pod_placement(&self, placement: &PodPlacement) -> StorageResult<()> {
        RuntimeStore::upsert_pod_placement(self, placement)
    }
    async fn upsert_host_net_assoc(&self, assoc: &HostNetAssoc) -> StorageResult<()> {
        RuntimeStore::upsert_host_net_assoc(self, assoc)
    }
    async fn list_host_net_assocs(&self, host_id: Uuid, page: Page) -> StorageResult<Vec<HostNetAssoc>> {
        RuntimeStore::list_host_net_assocs(self, host_id, page)
    }
}

pub trait GovernanceStore {
    fn upsert_responsibility_assignment(
        &self,
        assignment: &ResponsibilityAssignment,
    ) -> StorageResult<()>;

    fn get_responsibility_assignment(
        &self,
        assignment_id: Uuid,
    ) -> StorageResult<Option<ResponsibilityAssignment>>;

    fn list_responsibility_assignments_for_target(
        &self,
        target_kind: ObjectKind,
        target_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ResponsibilityAssignment>>;
}

#[allow(async_fn_in_trait)]
pub trait AsyncGovernanceStore: GovernanceStore + Sync {
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
        GovernanceStore::list_responsibility_assignments_for_target(self, target_kind, target_id, page)
    }
}

pub trait SyncStore {
    fn upsert_external_identity_link(&self, link: &ExternalIdentityLink) -> StorageResult<()>;
    fn get_external_identity_link(
        &self,
        link_id: Uuid,
    ) -> StorageResult<Option<ExternalIdentityLink>>;

    fn upsert_external_sync_cursor(&self, cursor: &ExternalSyncCursor) -> StorageResult<()>;
    fn get_external_sync_cursor(
        &self,
        cursor_id: Uuid,
    ) -> StorageResult<Option<ExternalSyncCursor>>;
}

#[allow(async_fn_in_trait)]
pub trait AsyncSyncStore: SyncStore + Sync {
    async fn upsert_external_identity_link(&self, link: &ExternalIdentityLink) -> StorageResult<()> {
        SyncStore::upsert_external_identity_link(self, link)
    }
    async fn get_external_identity_link(
        &self,
        link_id: Uuid,
    ) -> StorageResult<Option<ExternalIdentityLink>> {
        SyncStore::get_external_identity_link(self, link_id)
    }
    async fn upsert_external_sync_cursor(&self, cursor: &ExternalSyncCursor) -> StorageResult<()> {
        SyncStore::upsert_external_sync_cursor(self, cursor)
    }
    async fn get_external_sync_cursor(
        &self,
        cursor_id: Uuid,
    ) -> StorageResult<Option<ExternalSyncCursor>> {
        SyncStore::get_external_sync_cursor(self, cursor_id)
    }
}
