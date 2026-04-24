use topology_domain::{
    BusinessDomain, ClusterInventory, ExternalIdentityLink, ExternalSyncCursor, HostInventory,
    HostRuntimeState, NamespaceInventory, ObjectKind, PodInventory, PodPlacement,
    ResponsibilityAssignment, RuntimeBinding, ServiceEntity, ServiceInstance, Subject, Subsystem,
    SystemBoundary, TenantId, WorkloadEntity, WorkloadPodMembership,
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
    fn list_businesses(&self, tenant_id: TenantId, page: Page)
        -> StorageResult<Vec<BusinessDomain>>;

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

    fn upsert_subject(&self, subject: &Subject) -> StorageResult<()>;
    fn get_subject(&self, subject_id: Uuid) -> StorageResult<Option<Subject>>;
    fn list_subjects(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<Subject>>;
}

pub trait RuntimeStore {
    fn insert_host_runtime_state(&self, state: &HostRuntimeState) -> StorageResult<()>;
    fn list_host_runtime_states(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<HostRuntimeState>>;

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
