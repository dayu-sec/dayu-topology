use chrono::Utc;
use orion_error::conversion::ConvErr;
use topology_domain::{
    EffectiveResponsibilityView, HostProcessGroupView, HostTopologyView, NetworkTopologyView,
    ProcessRuntimeState, ProcessStateCount,
};
use topology_storage::{
    AsyncCatalogStore, AsyncGovernanceStore, AsyncRuntimeStore, CatalogStore, GovernanceStore,
    Page, RuntimeStore,
};
use uuid::Uuid;

use crate::error::ApiResult;

pub struct TopologyQueryService<S> {
    store: S,
}

impl<S> TopologyQueryService<S>
where
    S: CatalogStore + RuntimeStore + GovernanceStore,
{
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn host_topology_view(&self, host_id: Uuid) -> ApiResult<Option<HostTopologyView>> {
        let host = self.store.get_host(host_id).conv_err()?;
        let Some(host) = host else {
            return Ok(None);
        };

        let network_assocs = self
            .store
            .list_host_net_assocs(host_id, Page::default())
            .conv_err()?;
        let mut network_segments = Vec::new();
        for assoc in &network_assocs {
            if let Some(segment) = self
                .store
                .get_network_segment(assoc.network_segment_id)
                .conv_err()?
            {
                network_segments.push(segment);
            }
        }

        let assignments = self
            .store
            .list_responsibility_assignments_for_target(
                topology_domain::ObjectKind::Host,
                host.host_id,
                Page::default(),
            )
            .conv_err()?;
        let processes = list_all_process_runtime_states(&self.store, host.host_id)?;
        let latest_runtime = self
            .store
            .list_host_runtime_states(host.host_id, Page::default())
            .conv_err()?
            .into_iter()
            .max_by_key(|state| state.observed_at.0);

        Ok(Some(HostTopologyView {
            host,
            latest_runtime,
            process_groups: build_process_groups(&processes),
            processes,
            network_segments,
            network_assocs,
            services: Vec::new(),
            assignments,
            generated_at: Utc::now(),
        }))
    }

    pub fn host_process_topology_view(
        &self,
        host_id: Uuid,
    ) -> ApiResult<Option<HostTopologyView>> {
        self.host_topology_view(host_id)
    }

    pub fn network_topology_view(
        &self,
        network_segment_id: Uuid,
    ) -> ApiResult<Option<NetworkTopologyView>> {
        let segment = self
            .store
            .get_network_segment(network_segment_id)
            .conv_err()?;
        let Some(segment) = segment else {
            return Ok(None);
        };

        let mut hosts = Vec::new();
        let mut host_assocs = Vec::new();
        for host in self
            .store
            .list_hosts(segment.tenant_id, Page::default())
            .conv_err()?
        {
            let assocs = self
                .store
                .list_host_net_assocs(host.host_id, Page::default())
                .conv_err()?;
            let matched: Vec<_> = assocs
                .into_iter()
                .filter(|assoc| assoc.network_segment_id == network_segment_id)
                .collect();
            if !matched.is_empty() {
                hosts.push(host);
                host_assocs.extend(matched);
            }
        }

        let assignments = self
            .store
            .list_responsibility_assignments_for_target(
                topology_domain::ObjectKind::NetworkSegment,
                network_segment_id,
                Page::default(),
            )
            .conv_err()?;

        Ok(Some(NetworkTopologyView {
            segment,
            hosts,
            host_assocs,
            assignments,
            generated_at: Utc::now(),
        }))
    }

    pub fn effective_responsibility_view(
        &self,
        target_kind: topology_domain::ObjectKind,
        target_id: Uuid,
    ) -> ApiResult<Vec<EffectiveResponsibilityView>> {
        let assignments = self
            .store
            .list_responsibility_assignments_for_target(target_kind, target_id, Page::default())
            .conv_err()?;

        let mut views = Vec::new();
        for assignment in assignments {
            if let Some(subject) = self.store.get_subject(assignment.subject_id).conv_err()? {
                views.push(EffectiveResponsibilityView {
                    subject,
                    assignment,
                    generated_at: Utc::now(),
                });
            }
        }

        Ok(views)
    }
}

impl<S> TopologyQueryService<S>
where
    S: AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    pub async fn host_topology_view_async(
        &self,
        host_id: Uuid,
    ) -> ApiResult<Option<HostTopologyView>> {
        let host = AsyncCatalogStore::get_host(&self.store, host_id)
            .await
            .conv_err()?;
        let Some(host) = host else {
            return Ok(None);
        };

        let network_assocs = AsyncRuntimeStore::list_host_net_assocs(
            &self.store,
            host_id,
            Page::default(),
        )
            .await
            .conv_err()?;
        let mut network_segments = Vec::new();
        for assoc in &network_assocs {
            if let Some(segment) = AsyncCatalogStore::get_network_segment(
                &self.store,
                assoc.network_segment_id,
            )
                .await
                .conv_err()?
            {
                network_segments.push(segment);
            }
        }

        let assignments = AsyncGovernanceStore::list_responsibility_assignments_for_target(
            &self.store,
            topology_domain::ObjectKind::Host,
            host.host_id,
            Page::default(),
        )
            .await
            .conv_err()?;
        let processes = list_all_process_runtime_states_async(&self.store, host.host_id).await?;
        let latest_runtime = AsyncRuntimeStore::list_host_runtime_states(
            &self.store,
            host.host_id,
            Page::default(),
        )
            .await
            .conv_err()?
            .into_iter()
            .max_by_key(|state| state.observed_at.0);

        Ok(Some(HostTopologyView {
            host,
            latest_runtime,
            process_groups: build_process_groups(&processes),
            processes,
            network_segments,
            network_assocs,
            services: Vec::new(),
            assignments,
            generated_at: Utc::now(),
        }))
    }

    pub async fn host_process_topology_view_async(
        &self,
        host_id: Uuid,
    ) -> ApiResult<Option<HostTopologyView>> {
        self.host_topology_view_async(host_id).await
    }

    pub async fn network_topology_view_async(
        &self,
        network_segment_id: Uuid,
    ) -> ApiResult<Option<NetworkTopologyView>> {
        let segment = AsyncCatalogStore::get_network_segment(&self.store, network_segment_id)
            .await
            .conv_err()?;
        let Some(segment) = segment else {
            return Ok(None);
        };

        let mut hosts = Vec::new();
        let mut host_assocs = Vec::new();
        for host in AsyncCatalogStore::list_hosts(&self.store, segment.tenant_id, Page::default())
            .await
            .conv_err()?
        {
            let assocs = AsyncRuntimeStore::list_host_net_assocs(
                &self.store,
                host.host_id,
                Page::default(),
            )
                .await
                .conv_err()?;
            let matched: Vec<_> = assocs
                .into_iter()
                .filter(|assoc| assoc.network_segment_id == network_segment_id)
                .collect();
            if !matched.is_empty() {
                hosts.push(host);
                host_assocs.extend(matched);
            }
        }

        let assignments = AsyncGovernanceStore::list_responsibility_assignments_for_target(
            &self.store,
            topology_domain::ObjectKind::NetworkSegment,
            network_segment_id,
            Page::default(),
        )
            .await
            .conv_err()?;

        Ok(Some(NetworkTopologyView {
            segment,
            hosts,
            host_assocs,
            assignments,
            generated_at: Utc::now(),
        }))
    }

    pub async fn effective_responsibility_view_async(
        &self,
        target_kind: topology_domain::ObjectKind,
        target_id: Uuid,
    ) -> ApiResult<Vec<EffectiveResponsibilityView>> {
        let assignments = AsyncGovernanceStore::list_responsibility_assignments_for_target(
            &self.store,
            target_kind,
            target_id,
            Page::default(),
        )
            .await
            .conv_err()?;

        let mut views = Vec::new();
        for assignment in assignments {
            if let Some(subject) = AsyncCatalogStore::get_subject(
                &self.store,
                assignment.subject_id,
            )
                .await
                .conv_err()?
            {
                views.push(EffectiveResponsibilityView {
                    subject,
                    assignment,
                    generated_at: Utc::now(),
                });
            }
        }

        Ok(views)
    }
}

fn list_all_process_runtime_states<S>(store: &S, host_id: Uuid) -> ApiResult<Vec<topology_domain::ProcessRuntimeState>>
where
    S: RuntimeStore,
{
    let mut items = Vec::new();
    let page_limit = 500;
    let mut offset = 0;

    loop {
        let batch = store
            .list_process_runtime_states(
                host_id,
                Page {
                    limit: page_limit,
                    offset,
                },
            )
            .conv_err()?;
        let batch_len = batch.len();
        if batch_len == 0 {
            break;
        }
        items.extend(batch);
        if batch_len < page_limit as usize {
            break;
        }
        offset += batch_len as u32;
    }

    Ok(items)
}

async fn list_all_process_runtime_states_async<S>(
    store: &S,
    host_id: Uuid,
) -> ApiResult<Vec<topology_domain::ProcessRuntimeState>>
where
    S: AsyncRuntimeStore,
{
    let mut items = Vec::new();
    let page_limit = 500;
    let mut offset = 0;

    loop {
        let batch = AsyncRuntimeStore::list_process_runtime_states(
            store,
            host_id,
            Page {
                limit: page_limit,
                offset,
            },
        )
            .await
            .conv_err()?;
        let batch_len = batch.len();
        if batch_len == 0 {
            break;
        }
        items.extend(batch);
        if batch_len < page_limit as usize {
            break;
        }
        offset += batch_len as u32;
    }

    Ok(items)
}

fn build_process_groups(processes: &[ProcessRuntimeState]) -> Vec<HostProcessGroupView> {
    let mut grouped: std::collections::BTreeMap<String, Vec<&ProcessRuntimeState>> =
        std::collections::BTreeMap::new();
    for process in processes {
        grouped
            .entry(process.executable.clone())
            .or_default()
            .push(process);
    }

    let mut views: Vec<_> = grouped
        .into_iter()
        .map(|(executable, members)| {
            let process_count = members.len();
            let total_memory_rss_kib = members.iter().filter_map(|item| item.memory_rss_kib).sum();

            let mut state_counts: std::collections::BTreeMap<String, usize> =
                std::collections::BTreeMap::new();
            for member in &members {
                let state = member
                    .process_state
                    .clone()
                    .unwrap_or_else(|| "-".to_string());
                *state_counts.entry(state).or_insert(0) += 1;
            }

            let mut state_summary: Vec<ProcessStateCount> = state_counts
                .into_iter()
                .map(|(state, count)| ProcessStateCount { state, count })
                .collect();
            state_summary.sort_by(|left, right| {
                right
                    .count
                    .cmp(&left.count)
                    .then(left.state.cmp(&right.state))
            });

            HostProcessGroupView {
                display_name: executable
                    .rsplit('/')
                    .next()
                    .filter(|item| !item.is_empty())
                    .unwrap_or(executable.as_str())
                    .to_string(),
                executable,
                process_count,
                total_memory_rss_kib,
                dominant_state: state_summary.first().map(|item| item.state.clone()),
                state_summary,
            }
        })
        .collect();

    views.sort_by(|left, right| {
        right
            .process_count
            .cmp(&left.process_count)
            .then(right.total_memory_rss_kib.cmp(&left.total_memory_rss_kib))
            .then(left.display_name.cmp(&right.display_name))
    });
    views
}
