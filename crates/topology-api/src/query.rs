use chrono::Utc;
use orion_error::conversion::ConvErr;
use topology_domain::{
    EffectiveResponsibilityView, HostProcessGroupView, HostProcessGroupsPageView,
    HostProcessOverviewView, HostServiceInstanceView, HostServiceView, HostTopologyView,
    NetworkTopologyView, ProcessRuntimeState, ProcessStateCount, RuntimeObjectType,
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

impl<S> TopologyQueryService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

impl<S> TopologyQueryService<S>
where
    S: CatalogStore + RuntimeStore + GovernanceStore,
{
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
        let services = build_host_services(&self.store, &processes)?;
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
            services,
            assignments,
            generated_at: Utc::now(),
        }))
    }

    pub fn host_process_topology_view(&self, host_id: Uuid) -> ApiResult<Option<HostTopologyView>> {
        self.host_topology_view(host_id)
    }

    pub fn host_process_overview_view(
        &self,
        host_id: Uuid,
        top_n: usize,
    ) -> ApiResult<Option<HostProcessOverviewView>> {
        let Some(view) = self.host_topology_view(host_id)? else {
            return Ok(None);
        };
        Ok(Some(build_host_process_overview_view(view, top_n)))
    }

    pub fn host_process_groups_page_view(
        &self,
        host_id: Uuid,
        offset: usize,
        limit: usize,
    ) -> ApiResult<Option<HostProcessGroupsPageView>> {
        let Some(view) = self.host_topology_view(host_id)? else {
            return Ok(None);
        };
        Ok(Some(build_host_process_groups_page_view(
            view, offset, limit,
        )))
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

        let network_assocs =
            AsyncRuntimeStore::list_host_net_assocs(&self.store, host_id, Page::default())
                .await
                .conv_err()?;
        let mut network_segments = Vec::new();
        for assoc in &network_assocs {
            if let Some(segment) =
                AsyncCatalogStore::get_network_segment(&self.store, assoc.network_segment_id)
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
        let services = build_host_services_async(&self.store, &processes).await?;
        let latest_runtime =
            AsyncRuntimeStore::list_host_runtime_states(&self.store, host.host_id, Page::default())
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
            services,
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

    pub async fn host_process_overview_view_async(
        &self,
        host_id: Uuid,
        top_n: usize,
    ) -> ApiResult<Option<HostProcessOverviewView>> {
        let Some(view) = self.host_topology_view_async(host_id).await? else {
            return Ok(None);
        };
        Ok(Some(build_host_process_overview_view(view, top_n)))
    }

    pub async fn host_process_groups_page_view_async(
        &self,
        host_id: Uuid,
        offset: usize,
        limit: usize,
    ) -> ApiResult<Option<HostProcessGroupsPageView>> {
        let Some(view) = self.host_topology_view_async(host_id).await? else {
            return Ok(None);
        };
        Ok(Some(build_host_process_groups_page_view(
            view, offset, limit,
        )))
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
            let assocs =
                AsyncRuntimeStore::list_host_net_assocs(&self.store, host.host_id, Page::default())
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
            if let Some(subject) =
                AsyncCatalogStore::get_subject(&self.store, assignment.subject_id)
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

fn list_all_process_runtime_states<S>(
    store: &S,
    host_id: Uuid,
) -> ApiResult<Vec<topology_domain::ProcessRuntimeState>>
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

fn build_host_services<S>(
    store: &S,
    processes: &[ProcessRuntimeState],
) -> ApiResult<Vec<HostServiceView>>
where
    S: CatalogStore + RuntimeStore,
{
    let mut views = Vec::new();
    let mut seen_instance_ids = std::collections::BTreeSet::new();
    let process_by_id: std::collections::BTreeMap<Uuid, ProcessRuntimeState> = processes
        .iter()
        .cloned()
        .map(|process| (process.process_id, process))
        .collect();

    for process in processes {
        let bindings = store
            .list_runtime_bindings_for_object(
                RuntimeObjectType::Process,
                process.process_id,
                Page::default(),
            )
            .conv_err()?;
        for binding in bindings {
            if !seen_instance_ids.insert(binding.instance_id) {
                continue;
            }
            let Some(instance) = store.get_service_instance(binding.instance_id).conv_err()? else {
                continue;
            };
            let Some(service) = store.get_service(instance.service_id).conv_err()? else {
                continue;
            };

            let instance_bindings = store
                .list_runtime_bindings_for_instance(instance.instance_id, Page::default())
                .conv_err()?;
            let instance_processes = collect_bound_processes(&process_by_id, &instance_bindings);
            push_host_service_view(
                &mut views,
                service,
                HostServiceInstanceView {
                    instance,
                    bindings: instance_bindings,
                    processes: instance_processes,
                },
            );
        }
    }

    sort_host_service_views(&mut views);
    Ok(views)
}

async fn build_host_services_async<S>(
    store: &S,
    processes: &[ProcessRuntimeState],
) -> ApiResult<Vec<HostServiceView>>
where
    S: AsyncCatalogStore + AsyncRuntimeStore,
{
    let mut views = Vec::new();
    let mut seen_instance_ids = std::collections::BTreeSet::new();
    let process_by_id: std::collections::BTreeMap<Uuid, ProcessRuntimeState> = processes
        .iter()
        .cloned()
        .map(|process| (process.process_id, process))
        .collect();

    for process in processes {
        let bindings = AsyncRuntimeStore::list_runtime_bindings_for_object(
            store,
            RuntimeObjectType::Process,
            process.process_id,
            Page::default(),
        )
        .await
        .conv_err()?;
        for binding in bindings {
            if !seen_instance_ids.insert(binding.instance_id) {
                continue;
            }
            let Some(instance) =
                AsyncRuntimeStore::get_service_instance(store, binding.instance_id)
                    .await
                    .conv_err()?
            else {
                continue;
            };
            let Some(service) = AsyncCatalogStore::get_service(store, instance.service_id)
                .await
                .conv_err()?
            else {
                continue;
            };

            let instance_bindings = AsyncRuntimeStore::list_runtime_bindings_for_instance(
                store,
                instance.instance_id,
                Page::default(),
            )
            .await
            .conv_err()?;
            let instance_processes = collect_bound_processes(&process_by_id, &instance_bindings);
            push_host_service_view(
                &mut views,
                service,
                HostServiceInstanceView {
                    instance,
                    bindings: instance_bindings,
                    processes: instance_processes,
                },
            );
        }
    }

    sort_host_service_views(&mut views);
    Ok(views)
}

fn collect_bound_processes(
    process_by_id: &std::collections::BTreeMap<Uuid, ProcessRuntimeState>,
    bindings: &[topology_domain::RuntimeBinding],
) -> Vec<ProcessRuntimeState> {
    let mut items = Vec::new();
    for binding in bindings {
        if binding.object_type != RuntimeObjectType::Process {
            continue;
        }
        if let Some(process) = process_by_id.get(&binding.object_id) {
            items.push(process.clone());
        }
    }
    items.sort_by(|left, right| {
        left.pid
            .cmp(&right.pid)
            .then(left.process_id.cmp(&right.process_id))
    });
    items
}

fn push_host_service_view(
    views: &mut Vec<HostServiceView>,
    service: topology_domain::ServiceEntity,
    instance_view: HostServiceInstanceView,
) {
    if let Some(existing) = views
        .iter_mut()
        .find(|item| item.service.service_id == service.service_id)
    {
        existing.instances.push(instance_view);
        return;
    }
    views.push(HostServiceView {
        service,
        instances: vec![instance_view],
    });
}

fn sort_host_service_views(views: &mut [HostServiceView]) {
    for view in views.iter_mut() {
        view.instances.sort_by(|left, right| {
            right
                .instance
                .last_seen_at
                .cmp(&left.instance.last_seen_at)
                .then(left.instance.instance_id.cmp(&right.instance.instance_id))
        });
    }
    views.sort_by(|left, right| left.service.name.cmp(&right.service.name));
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

fn build_host_process_overview_view(
    view: HostTopologyView,
    top_n: usize,
) -> HostProcessOverviewView {
    let total_processes = view.processes.len();
    let total_groups = view.process_groups.len();
    let top_groups = view
        .process_groups
        .into_iter()
        .take(top_n)
        .collect::<Vec<_>>();
    let truncated_group_count = total_groups.saturating_sub(top_groups.len());

    HostProcessOverviewView {
        host: view.host,
        total_processes,
        total_groups,
        top_groups,
        truncated_group_count,
        generated_at: view.generated_at,
    }
}

fn build_host_process_groups_page_view(
    view: HostTopologyView,
    offset: usize,
    limit: usize,
) -> HostProcessGroupsPageView {
    let total_processes = view.processes.len();
    let total_groups = view.process_groups.len();
    let groups = view
        .process_groups
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();
    let has_more = offset.saturating_add(groups.len()) < total_groups;

    HostProcessGroupsPageView {
        host: view.host,
        total_processes,
        total_groups,
        groups,
        limit,
        offset,
        has_more,
        generated_at: view.generated_at,
    }
}
