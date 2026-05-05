use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use topology_domain::{
    HostInventory, HostNetAssoc, NetworkDomain, NetworkSegment, ResponsibilityAssignment, Subject,
    TenantId,
};
use uuid::Uuid;

use crate::{
    CatalogStore, GovernanceStore, Page, RuntimeStore, StorageResult, not_configured,
    operation_failed,
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

#[derive(Debug, Default)]
struct MemoryState {
    hosts: Vec<HostInventory>,
    network_domains: Vec<NetworkDomain>,
    network_segments: Vec<NetworkSegment>,
    host_net_assocs: Vec<HostNetAssoc>,
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

        store.upsert_host(&host).unwrap();
        store.upsert_network_domain(&domain).unwrap();
        store.upsert_network_segment(&segment).unwrap();
        store
            .record_ingest_job(IngestJobEntry {
                ingest_id: "ing-1".to_string(),
                tenant_id,
                source_name: "fixture".to_string(),
                source_kind: "batch_import".to_string(),
                received_at: Utc::now(),
                status: "accepted".to_string(),
                payload_ref: None,
                error: None,
            })
            .unwrap();

        assert_eq!(
            store.list_hosts(tenant_id, Page::default()).unwrap().len(),
            1
        );
        assert_eq!(
            store
                .list_network_segments(tenant_id, Page::default())
                .unwrap()
                .len(),
            1
        );
        assert!(store.get_ingest_job("ing-1").unwrap().is_some());
    }
}
