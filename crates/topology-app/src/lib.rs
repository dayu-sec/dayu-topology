pub mod error;

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use axum::{
    Json, Router,
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::Utc;
use orion_error::{
    conversion::{ConvErr, ToStructError},
    prelude::*,
};
use serde::Serialize;
use serde_json::Value;
use topology_api::{TopologyIngestService, TopologyQueryService};
use topology_domain::{
    DayuInputEnvelope, IngestEnvelope, IngestMode, ObjectKind, SourceKind, TenantId,
};
use topology_storage::AsyncIngestStore;
use topology_storage::{
    AsyncCatalogStore, AsyncGovernanceStore, AsyncRuntimeStore, CatalogStore, GovernanceStore,
    InMemoryTopologyStore, IngestStore, LivePostgresExecutor, MemoryPostgresExecutor, Page,
    PostgresTopologyStore, RuntimeStore,
};
use topology_sync::{JsonlImportService, JsonlImportSummary};
use uuid::Uuid;

pub use error::{AppError, AppReason, AppResult};
use error::{invalid_args, materialization_missing};

pub enum TopologyMonolith {
    Memory(InMemoryTopologyStore),
    PostgresMock(PostgresTopologyStore<MemoryPostgresExecutor>),
    PostgresLive(PostgresTopologyStore<LivePostgresExecutor>),
}

pub struct TopologyAppBuilder {
    mode: MonolithMode,
}

pub struct MonolithRunSummary {
    pub ingest_id: String,
    pub host_name: String,
    pub network_name: Option<String>,
    pub assoc_ip: Option<String>,
    pub responsibilities: Vec<String>,
}

pub type JsonlReplaySummary = JsonlImportSummary;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonolithMode {
    Memory,
    PostgresMock,
    PostgresLive,
}

pub enum MonolithInput {
    Demo,
    File(PathBuf),
    JsonlFiles(Vec<PathBuf>),
    ResetPublic,
    ReplaceJsonlFiles(Vec<PathBuf>),
    ExportVisualization(PathBuf),
    PrintHostProcessTopology(Uuid),
    PrintFirstHostProcessTopology,
    Serve { listen: String },
}

pub enum MonolithRunResult {
    Single(MonolithRunSummary),
    Replay(JsonlReplaySummary),
    Reset(String),
    ExportVisualization(VisualizationExportSummary),
    PrintJson(String),
    Serve { listen: String },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VisualizationExportSummary {
    pub output_path: PathBuf,
    pub host_count: usize,
    pub process_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct VisualizationEnvelope {
    status: &'static str,
    data: HostProcessTopologyGraph,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostTopologyHttpEnvelope {
    status: &'static str,
    data: HostTopologyHttpView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostProcessOverviewHttpEnvelope {
    status: &'static str,
    data: HostProcessOverviewHttpView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostProcessGroupsPageHttpEnvelope {
    status: &'static str,
    data: HostProcessGroupsPageHttpView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostTopologyHttpView {
    host: HostTopologyHostDto,
    latest_runtime: Option<topology_domain::HostRuntimeState>,
    process_groups: Vec<topology_domain::HostProcessGroupView>,
    processes: Vec<topology_domain::ProcessRuntimeState>,
    network_segments: Vec<topology_domain::NetworkSegment>,
    network_assocs: Vec<topology_domain::HostNetAssoc>,
    services: Vec<topology_domain::HostServiceView>,
    assignments: Vec<topology_domain::ResponsibilityAssignment>,
    generated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostProcessOverviewHttpView {
    host: HostTopologyHostDto,
    total_processes: usize,
    total_groups: usize,
    top_groups: Vec<topology_domain::HostProcessGroupView>,
    truncated_group_count: usize,
    generated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostProcessGroupsPageHttpView {
    host: HostTopologyHostDto,
    total_processes: usize,
    total_groups: usize,
    groups: Vec<topology_domain::HostProcessGroupView>,
    limit: usize,
    offset: usize,
    has_more: bool,
    generated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostTopologyHostDto {
    id: Uuid,
    host_name: String,
    machine_id: Option<String>,
    os_name: Option<String>,
    os_version: Option<String>,
}

#[derive(Clone)]
struct HttpAppState {
    app: TopologyMonolith,
}

#[derive(Debug, Clone, Serialize)]
struct HostProcessTopologyGraph {
    nodes: Vec<HostProcessTopologyNode>,
    edges: Vec<HostProcessTopologyEdge>,
    metadata: HostProcessTopologyMetadata,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostProcessTopologyNode {
    id: String,
    object_kind: &'static str,
    object_id: String,
    layer: &'static str,
    label: String,
    properties: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostProcessTopologyEdge {
    id: String,
    edge_kind: &'static str,
    source: String,
    target: String,
    label: Option<String>,
    properties: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostProcessTopologyMetadata {
    query_time: String,
    host_count: usize,
    process_count: usize,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct HostProcessOverviewQuery {
    top_n: Option<usize>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct HostProcessGroupsQuery {
    limit: Option<usize>,
    offset: Option<usize>,
}

impl TopologyMonolith {
    pub fn new_in_memory() -> Self {
        Self::Memory(InMemoryTopologyStore::default())
    }

    pub fn new_postgres_mock() -> AppResult<Self> {
        let store = PostgresTopologyStore::new(MemoryPostgresExecutor::default());
        store.run_migrations().conv_err()?;
        Ok(Self::PostgresMock(store))
    }

    pub async fn new_postgres_live(database_url: impl Into<String>) -> AppResult<Self> {
        let executor = LivePostgresExecutor::new(database_url).await.conv_err()?;
        let store = PostgresTopologyStore::new(executor);
        store.run_migrations_async().await.conv_err()?;
        Ok(Self::PostgresLive(store))
    }

    pub fn run(&self, input: MonolithInput) -> AppResult<MonolithRunResult> {
        match self {
            Self::Memory(store) => match input {
                MonolithInput::Serve { listen } => self.serve_http(listen),
                other => run_with_store(store.clone(), other),
            },
            Self::PostgresMock(store) => match input {
                MonolithInput::Serve { listen } => self.serve_http(listen),
                other => run_with_postgres_store(store.clone(), other),
            },
            Self::PostgresLive(_) => Err(materialization_missing(
                "postgres-live requires async run entrypoint",
            )),
        }
    }

    pub fn serve_http(&self, listen: impl Into<String>) -> AppResult<MonolithRunResult> {
        let listen = listen.into();
        let addr: SocketAddr = listen.parse().map_err(|err| {
            AppReason::InvalidArgs
                .to_err()
                .with_detail(format!("parse listen address {listen}: {err}"))
        })?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|err| {
                AppReason::InputLoadFailed
                    .to_err()
                    .with_detail(format!("build tokio runtime: {err}"))
            })?;

        let state = HttpAppState { app: self.clone() };
        runtime.block_on(async move {
            let router = build_http_router(state);

            let listener = tokio::net::TcpListener::bind(addr).await.map_err(|err| {
                AppReason::InputLoadFailed
                    .to_err()
                    .with_detail(format!("bind {addr}: {err}"))
            })?;
            axum::serve(listener, router).await.map_err(|err| {
                AppReason::InputLoadFailed
                    .to_err()
                    .with_detail(format!("serve http: {err}"))
            })?;
            Ok::<(), AppError>(())
        })?;

        Ok(MonolithRunResult::Serve { listen })
    }

    pub fn run_demo(&self) -> AppResult<MonolithRunSummary> {
        match self.run(MonolithInput::Demo)? {
            MonolithRunResult::Single(summary) => Ok(summary),
            MonolithRunResult::Replay(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::Reset(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::ExportVisualization(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::PrintJson(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::Serve { .. } => {
                Err(materialization_missing("expected single run summary"))
            }
        }
    }

    pub fn run_file(&self, path: impl AsRef<Path>) -> AppResult<MonolithRunSummary> {
        match self.run(MonolithInput::File(path.as_ref().to_path_buf()))? {
            MonolithRunResult::Single(summary) => Ok(summary),
            MonolithRunResult::Replay(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::Reset(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::ExportVisualization(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::PrintJson(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::Serve { .. } => {
                Err(materialization_missing("expected single run summary"))
            }
        }
    }

    pub fn replay_jsonl(&self, path: impl AsRef<Path>) -> AppResult<JsonlReplaySummary> {
        match self.run(MonolithInput::JsonlFiles(vec![path.as_ref().to_path_buf()]))? {
            MonolithRunResult::Replay(summary) => Ok(summary),
            MonolithRunResult::Single(_) => Err(materialization_missing("expected replay summary")),
            MonolithRunResult::Reset(_) => Err(materialization_missing("expected replay summary")),
            MonolithRunResult::ExportVisualization(_) => {
                Err(materialization_missing("expected replay summary"))
            }
            MonolithRunResult::PrintJson(_) => {
                Err(materialization_missing("expected replay summary"))
            }
            MonolithRunResult::Serve { .. } => {
                Err(materialization_missing("expected replay summary"))
            }
        }
    }

    pub fn replay_jsonl_files(&self, paths: Vec<PathBuf>) -> AppResult<JsonlReplaySummary> {
        match self.run(MonolithInput::JsonlFiles(paths))? {
            MonolithRunResult::Replay(summary) => Ok(summary),
            MonolithRunResult::Single(_) => Err(materialization_missing("expected replay summary")),
            MonolithRunResult::Reset(_) => Err(materialization_missing("expected replay summary")),
            MonolithRunResult::ExportVisualization(_) => {
                Err(materialization_missing("expected replay summary"))
            }
            MonolithRunResult::PrintJson(_) => {
                Err(materialization_missing("expected replay summary"))
            }
            MonolithRunResult::Serve { .. } => {
                Err(materialization_missing("expected replay summary"))
            }
        }
    }

    pub async fn run_async(&self, input: MonolithInput) -> AppResult<MonolithRunResult> {
        match self {
            Self::Memory(store) => match input {
                MonolithInput::Serve { listen } => self.serve_http_async(listen).await,
                other => run_with_store(store.clone(), other),
            },
            Self::PostgresMock(store) => match input {
                MonolithInput::Serve { listen } => self.serve_http_async(listen).await,
                other => run_with_postgres_store(store.clone(), other),
            },
            Self::PostgresLive(store) => match input {
                MonolithInput::Serve { listen } => self.serve_http_async(listen).await,
                other => run_with_postgres_store_async(store.clone(), other).await,
            },
        }
    }

    pub async fn serve_http_async(
        &self,
        listen: impl Into<String>,
    ) -> AppResult<MonolithRunResult> {
        let listen = listen.into();
        let addr: SocketAddr = listen.parse().map_err(|err| {
            AppReason::InvalidArgs
                .to_err()
                .with_detail(format!("parse listen address {listen}: {err}"))
        })?;

        let state = HttpAppState { app: self.clone() };
        let router = build_http_router(state);

        let listener = tokio::net::TcpListener::bind(addr).await.map_err(|err| {
            AppReason::InputLoadFailed
                .to_err()
                .with_detail(format!("bind {addr}: {err}"))
        })?;
        axum::serve(listener, router).await.map_err(|err| {
            AppReason::InputLoadFailed
                .to_err()
                .with_detail(format!("serve http: {err}"))
        })?;

        Ok(MonolithRunResult::Serve { listen })
    }

    pub async fn run_demo_async(&self) -> AppResult<MonolithRunSummary> {
        match self.run_async(MonolithInput::Demo).await? {
            MonolithRunResult::Single(summary) => Ok(summary),
            MonolithRunResult::Replay(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::Reset(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::ExportVisualization(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::PrintJson(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::Serve { .. } => {
                Err(materialization_missing("expected single run summary"))
            }
        }
    }

    pub fn store(&self) -> Option<&InMemoryTopologyStore> {
        match self {
            Self::Memory(store) => Some(store),
            Self::PostgresMock(_) | Self::PostgresLive(_) => None,
        }
    }
}

impl Clone for TopologyMonolith {
    fn clone(&self) -> Self {
        match self {
            Self::Memory(store) => Self::Memory(store.clone()),
            Self::PostgresMock(store) => Self::PostgresMock(store.clone()),
            Self::PostgresLive(store) => Self::PostgresLive(store.clone()),
        }
    }
}

impl TopologyAppBuilder {
    pub fn new() -> Self {
        Self {
            mode: MonolithMode::Memory,
        }
    }

    pub fn with_mode(mut self, mode: MonolithMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn build(self) -> AppResult<TopologyMonolith> {
        match self.mode {
            MonolithMode::Memory => Ok(TopologyMonolith::new_in_memory()),
            MonolithMode::PostgresMock => TopologyMonolith::new_postgres_mock(),
            MonolithMode::PostgresLive => Err(materialization_missing(
                "postgres-live requires async build entrypoint",
            )),
        }
    }

    pub async fn build_async(self) -> AppResult<TopologyMonolith> {
        match self.mode {
            MonolithMode::Memory => Ok(TopologyMonolith::new_in_memory()),
            MonolithMode::PostgresMock => TopologyMonolith::new_postgres_mock(),
            MonolithMode::PostgresLive => {
                TopologyMonolith::new_postgres_live(resolve_database_url()).await
            }
        }
    }
}

impl Default for TopologyAppBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn run_with_store<S>(store: S, input: MonolithInput) -> AppResult<MonolithRunResult>
where
    S: Clone + CatalogStore + RuntimeStore + GovernanceStore + IngestStore,
{
    let tenant_id = TenantId(Uuid::new_v4());
    match input {
        MonolithInput::Demo => run_with_payload(store, tenant_id, load_demo_payload()?),
        MonolithInput::File(path) => {
            run_with_payload(store, tenant_id, load_payload_from_file(&path)?)
        }
        MonolithInput::JsonlFiles(paths) => run_with_jsonl_files(store, tenant_id, paths),
        MonolithInput::ExportVisualization(output_path) => {
            export_visualization_for_store(store, output_path)
        }
        MonolithInput::PrintHostProcessTopology(host_id) => {
            print_host_process_topology_for_store(store, Some(host_id))
        }
        MonolithInput::PrintFirstHostProcessTopology => {
            print_host_process_topology_for_store(store, None)
        }
        MonolithInput::Serve { .. } => Err(invalid_args()),
        MonolithInput::ResetPublic | MonolithInput::ReplaceJsonlFiles(_) => Err(invalid_args()),
    }
}

fn run_with_postgres_store<E>(
    store: PostgresTopologyStore<E>,
    input: MonolithInput,
) -> AppResult<MonolithRunResult>
where
    E: topology_storage::PostgresExecutor,
    PostgresTopologyStore<E>:
        AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore + AsyncIngestStore,
{
    match input {
        MonolithInput::JsonlFiles(paths) => {
            let tenant_id = TenantId(Uuid::new_v4());
            run_with_jsonl_files(store, tenant_id, paths)
        }
        MonolithInput::ResetPublic => {
            store.reset_public_schema().conv_err()?;
            Ok(MonolithRunResult::Reset(
                "public schema reset complete".to_string(),
            ))
        }
        MonolithInput::ReplaceJsonlFiles(paths) => {
            store.reset_public_schema().conv_err()?;
            let tenant_id = TenantId(Uuid::new_v4());
            run_with_jsonl_files(store, tenant_id, paths)
        }
        other => run_with_store(store, other),
    }
}

async fn run_with_postgres_store_async(
    store: PostgresTopologyStore<LivePostgresExecutor>,
    input: MonolithInput,
) -> AppResult<MonolithRunResult> {
    match input {
        MonolithInput::Demo => {
            run_with_payload_async(store, TenantId(Uuid::new_v4()), load_demo_payload()?).await
        }
        MonolithInput::File(path) => {
            run_with_payload_async(
                store,
                TenantId(Uuid::new_v4()),
                load_payload_from_file(&path)?,
            )
            .await
        }
        MonolithInput::JsonlFiles(paths) => {
            let tenant_id = TenantId(Uuid::new_v4());
            let summary = JsonlImportService::new(store)
                .import_files_async(tenant_id, &paths)
                .await
                .conv_err()?;
            Ok(MonolithRunResult::Replay(summary))
        }
        MonolithInput::ResetPublic => {
            store.reset_public_schema_async().await.conv_err()?;
            Ok(MonolithRunResult::Reset(
                "public schema reset complete".to_string(),
            ))
        }
        MonolithInput::ReplaceJsonlFiles(paths) => {
            store.reset_public_schema_async().await.conv_err()?;
            let tenant_id = TenantId(Uuid::new_v4());
            let summary = JsonlImportService::new(store)
                .import_files_async(tenant_id, &paths)
                .await
                .conv_err()?;
            Ok(MonolithRunResult::Replay(summary))
        }
        MonolithInput::ExportVisualization(output_path) => {
            export_visualization_for_store_async(store, output_path).await
        }
        MonolithInput::PrintHostProcessTopology(host_id) => {
            print_host_process_topology_for_store_async(store, Some(host_id)).await
        }
        MonolithInput::PrintFirstHostProcessTopology => {
            print_host_process_topology_for_store_async(store, None).await
        }
        MonolithInput::Serve { .. } => Err(invalid_args()),
    }
}

fn run_with_payload<S>(
    store: S,
    tenant_id: TenantId,
    payload_inline: Value,
) -> AppResult<MonolithRunResult>
where
    S: Clone + CatalogStore + RuntimeStore + GovernanceStore + IngestStore,
{
    let ingest = TopologyIngestService::new(store.clone());
    let query = TopologyQueryService::new(store.clone());
    let envelope = ingest_envelope_from_input(payload_inline, tenant_id)?;

    let (record, _) = ingest.submit_and_materialize(envelope).conv_err()?;
    let host = store
        .list_hosts(tenant_id, Page::default())
        .conv_err()?
        .into_iter()
        .next()
        .ok_or_else(|| materialization_missing("host was not materialized"))?;
    let host_view = query
        .host_topology_view(host.host_id)
        .conv_err()?
        .ok_or_else(|| materialization_missing("host topology view was not built"))?;
    let assoc = host_view.network_assocs.first();
    let network = host_view.network_segments.first();
    let responsibility_views = query
        .effective_responsibility_view(ObjectKind::Host, host.host_id)
        .conv_err()?;

    Ok(MonolithRunResult::Single(MonolithRunSummary {
        ingest_id: record.ingest_id,
        host_name: host_view.host.host_name,
        network_name: network.map(|item| item.name.clone()),
        assoc_ip: assoc.map(|item| item.ip_addr.clone()),
        responsibilities: responsibility_views
            .into_iter()
            .map(|view| format!("{}:{:?}", view.subject.display_name, view.assignment.role))
            .collect(),
    }))
}

fn run_with_jsonl_files<S>(
    store: S,
    tenant_id: TenantId,
    paths: Vec<PathBuf>,
) -> AppResult<MonolithRunResult>
where
    S: Clone + CatalogStore + RuntimeStore + GovernanceStore + IngestStore,
{
    let summary = JsonlImportService::new(store)
        .import_files(tenant_id, &paths)
        .conv_err()?;
    Ok(MonolithRunResult::Replay(summary))
}

async fn run_with_payload_async<S>(
    store: S,
    tenant_id: TenantId,
    payload_inline: Value,
) -> AppResult<MonolithRunResult>
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore + AsyncIngestStore,
{
    let ingest = TopologyIngestService::new(store.clone());
    let query = TopologyQueryService::new(store.clone());
    let envelope = ingest_envelope_from_input(payload_inline, tenant_id)?;

    let (record, _) = ingest
        .submit_and_materialize_async(envelope)
        .await
        .conv_err()?;
    let host = AsyncCatalogStore::list_hosts(&store, tenant_id, Page::default())
        .await
        .conv_err()?
        .into_iter()
        .next()
        .ok_or_else(|| materialization_missing("host was not materialized"))?;
    let host_view = query
        .host_topology_view_async(host.host_id)
        .await
        .conv_err()?
        .ok_or_else(|| materialization_missing("host topology view was not built"))?;
    let assoc = host_view.network_assocs.first();
    let network = host_view.network_segments.first();
    let responsibility_views = query
        .effective_responsibility_view_async(ObjectKind::Host, host.host_id)
        .await
        .conv_err()?;

    Ok(MonolithRunResult::Single(MonolithRunSummary {
        ingest_id: record.ingest_id,
        host_name: host_view.host.host_name,
        network_name: network.map(|item| item.name.clone()),
        assoc_ip: assoc.map(|item| item.ip_addr.clone()),
        responsibilities: responsibility_views
            .into_iter()
            .map(|view| format!("{}:{:?}", view.subject.display_name, view.assignment.role))
            .collect(),
    }))
}

fn export_visualization_for_store<S>(store: S, output_path: PathBuf) -> AppResult<MonolithRunResult>
where
    S: Clone + CatalogStore + RuntimeStore + GovernanceStore,
{
    let graph = build_visualization_graph(store)?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).source_err(
            AppReason::InputLoadFailed,
            format!("create {}", parent.display()),
        )?;
    }
    let payload = VisualizationEnvelope {
        status: "ok",
        data: graph,
    };
    let body = serde_json::to_string_pretty(&payload)
        .source_err(AppReason::InputLoadFailed, "serialize visualization export")?;
    fs::write(&output_path, body).source_err(
        AppReason::InputLoadFailed,
        format!("write {}", output_path.display()),
    )?;

    Ok(MonolithRunResult::ExportVisualization(
        VisualizationExportSummary {
            output_path,
            host_count: payload.data.metadata.host_count,
            process_count: payload.data.metadata.process_count,
        },
    ))
}

async fn export_visualization_for_store_async<S>(
    store: S,
    output_path: PathBuf,
) -> AppResult<MonolithRunResult>
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    let graph = build_host_process_topology_graph_async(store, None).await?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).source_err(
            AppReason::InputLoadFailed,
            format!("create {}", parent.display()),
        )?;
    }
    let payload = VisualizationEnvelope {
        status: "ok",
        data: graph,
    };
    let body = serde_json::to_string_pretty(&payload)
        .source_err(AppReason::InputLoadFailed, "serialize visualization export")?;
    fs::write(&output_path, body).source_err(
        AppReason::InputLoadFailed,
        format!("write {}", output_path.display()),
    )?;

    Ok(MonolithRunResult::ExportVisualization(
        VisualizationExportSummary {
            output_path,
            host_count: payload.data.metadata.host_count,
            process_count: payload.data.metadata.process_count,
        },
    ))
}

fn print_host_process_topology_for_store<S>(
    store: S,
    host_id: Option<Uuid>,
) -> AppResult<MonolithRunResult>
where
    S: Clone + CatalogStore + RuntimeStore + GovernanceStore,
{
    let graph = build_host_process_topology_graph(store, host_id)?;
    let payload = VisualizationEnvelope {
        status: "ok",
        data: graph,
    };
    let body = serde_json::to_string_pretty(&payload).source_err(
        AppReason::InputLoadFailed,
        "serialize host process topology",
    )?;
    Ok(MonolithRunResult::PrintJson(body))
}

async fn print_host_process_topology_for_store_async<S>(
    store: S,
    host_id: Option<Uuid>,
) -> AppResult<MonolithRunResult>
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    let graph = build_host_process_topology_graph_async(store, host_id).await?;
    let payload = VisualizationEnvelope {
        status: "ok",
        data: graph,
    };
    let body = serde_json::to_string_pretty(&payload).source_err(
        AppReason::InputLoadFailed,
        "serialize host process topology",
    )?;
    Ok(MonolithRunResult::PrintJson(body))
}

fn ingest_envelope_from_input(input: Value, tenant_id: TenantId) -> AppResult<IngestEnvelope> {
    if looks_like_dayu_input(&input) {
        let dayu_input: DayuInputEnvelope = serde_json::from_value(input)
            .source_err(AppReason::InputLoadFailed, "parse dayu input")?;
        dayu_input.validate().conv_err()?;
        return Ok(dayu_input.into_ingest_envelope(tenant_id, None, Utc::now()));
    }

    Ok(IngestEnvelope {
        ingest_id: "demo-ingest-1".to_string(),
        source_kind: SourceKind::BatchImport,
        source_name: "monolith".to_string(),
        ingest_mode: IngestMode::BatchUpsert,
        tenant_id,
        environment_id: None,
        observed_at: None,
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(input),
        metadata: Default::default(),
    })
}

fn looks_like_dayu_input(input: &Value) -> bool {
    let Some(object) = input.as_object() else {
        return false;
    };

    object.contains_key("schema")
        || object.contains_key("source")
        || object.contains_key("collect")
        || object.contains_key("payload")
}

fn build_visualization_graph<S>(store: S) -> AppResult<HostProcessTopologyGraph>
where
    S: Clone + CatalogStore + RuntimeStore + GovernanceStore,
{
    build_host_process_topology_graph(store, None)
}

fn build_host_process_topology_graph<S>(
    store: S,
    focus_host_id: Option<Uuid>,
) -> AppResult<HostProcessTopologyGraph>
where
    S: Clone + CatalogStore + RuntimeStore + GovernanceStore,
{
    let mut hosts = Vec::new();
    if let Some(host_id) = focus_host_id {
        let host = store
            .get_host(host_id)
            .conv_err()?
            .ok_or_else(|| materialization_missing(format!("host {host_id} was not found")))?;
        hosts.push(host);
    } else {
        let mut offset = 0;
        let page_limit = 200;
        loop {
            let page = Page {
                limit: page_limit,
                offset,
            };
            let batch = list_all_hosts_page(&store, page)?;
            if batch.is_empty() {
                break;
            }
            offset += batch.len() as u32;
            hosts.extend(batch);
            if hosts.len() % page_limit as usize != 0 {
                break;
            }
        }
    }

    let query = TopologyQueryService::new(store.clone());
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut process_count = 0usize;

    for host in hosts {
        let Some(view) = query.host_topology_view(host.host_id).conv_err()? else {
            continue;
        };

        let host_node_id = format!("host:{}", view.host.host_id);
        let mut host_props = serde_json::Map::new();
        host_props.insert(
            "hostName".to_string(),
            Value::String(view.host.host_name.clone()),
        );
        if let Some(machine_id) = &view.host.machine_id {
            host_props.insert("machineId".to_string(), Value::String(machine_id.clone()));
        }
        if let Some(os_name) = &view.host.os_name {
            host_props.insert("osName".to_string(), Value::String(os_name.clone()));
        }
        if let Some(os_version) = &view.host.os_version {
            host_props.insert("osVersion".to_string(), Value::String(os_version.clone()));
        }
        if let Some(runtime) = &view.latest_runtime {
            host_props.insert(
                "observedAt".to_string(),
                Value::String(runtime.observed_at.0.to_rfc3339()),
            );
            if let Some(loadavg) = runtime.loadavg_1m {
                host_props.insert("loadavg1m".to_string(), Value::from(loadavg));
            }
            if let Some(memory_used_bytes) = runtime.memory_used_bytes {
                host_props.insert(
                    "memoryUsedBytes".to_string(),
                    Value::from(memory_used_bytes),
                );
            }
            if let Some(processes) = runtime.process_count {
                host_props.insert("processCount".to_string(), Value::from(processes));
            }
        }
        nodes.push(HostProcessTopologyNode {
            id: host_node_id.clone(),
            object_kind: "HostInventory",
            object_id: view.host.host_id.to_string(),
            layer: "resource",
            label: view.host.host_name.clone(),
            properties: host_props,
        });

        let summary_node_id = format!("process-summary:{}", host_node_id);
        let mut summary_props = serde_json::Map::new();
        summary_props.insert(
            "totalProcesses".to_string(),
            Value::from(view.processes.len() as i64),
        );
        summary_props.insert(
            "totalPrograms".to_string(),
            Value::from(view.process_groups.len() as i64),
        );
        summary_props.insert(
            "topPrograms".to_string(),
            Value::String(
                view.process_groups
                    .iter()
                    .take(5)
                    .map(|group| format!("{} x{}", group.display_name, group.process_count))
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        );
        nodes.push(HostProcessTopologyNode {
            id: summary_node_id.clone(),
            object_kind: "ProcessSummary",
            object_id: view.host.host_id.to_string(),
            layer: "resource",
            label: format!("processes: {}", view.processes.len()),
            properties: summary_props,
        });
        edges.push(HostProcessTopologyEdge {
            id: format!("edge:{}:process-summary", view.host.host_id),
            edge_kind: "host_process_assoc",
            source: host_node_id.clone(),
            target: summary_node_id.clone(),
            label: None,
            properties: serde_json::Map::new(),
        });

        for group in &view.process_groups {
            let group_node_id = format!("process-group:{}:{}", host_node_id, group.executable);
            let mut group_props = serde_json::Map::new();
            group_props.insert(
                "executable".to_string(),
                Value::String(group.executable.clone()),
            );
            group_props.insert(
                "processCount".to_string(),
                Value::from(group.process_count as i64),
            );
            group_props.insert(
                "totalMemoryRssKiB".to_string(),
                Value::from(group.total_memory_rss_kib),
            );
            group_props.insert(
                "totalMemoryRssMiB".to_string(),
                Value::from(((group.total_memory_rss_kib as f64) / 1024.0 * 10.0).round() / 10.0),
            );
            if let Some(state) = &group.dominant_state {
                group_props.insert("dominantState".to_string(), Value::String(state.clone()));
            }
            group_props.insert(
                "states".to_string(),
                Value::String(
                    group
                        .state_summary
                        .iter()
                        .take(3)
                        .map(|item| format!("{}:{}", item.state, item.count))
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            );
            nodes.push(HostProcessTopologyNode {
                id: group_node_id.clone(),
                object_kind: "ProcessGroup",
                object_id: view.host.host_id.to_string(),
                layer: "resource",
                label: format!("{} x{}", group.display_name, group.process_count),
                properties: group_props,
            });
            edges.push(HostProcessTopologyEdge {
                id: format!("edge:{}:{}", summary_node_id, group.executable),
                edge_kind: "host_process_assoc",
                source: summary_node_id.clone(),
                target: group_node_id.clone(),
                label: None,
                properties: serde_json::Map::new(),
            });
        }

        for process in view.processes {
            process_count += 1;
            let process_node_id = format!("process:{}", process.process_id);
            let mut process_props = serde_json::Map::new();
            process_props.insert("pid".to_string(), Value::from(process.pid));
            process_props.insert(
                "executable".to_string(),
                Value::String(process.executable.clone()),
            );
            if let Some(command_line) = &process.command_line {
                process_props.insert(
                    "commandLine".to_string(),
                    Value::String(command_line.clone()),
                );
            }
            if let Some(state) = &process.process_state {
                process_props.insert("processState".to_string(), Value::String(state.clone()));
            }
            if let Some(memory_rss_kib) = process.memory_rss_kib {
                process_props.insert("memoryRssKiB".to_string(), Value::from(memory_rss_kib));
            }
            process_props.insert(
                "observedAt".to_string(),
                Value::String(process.observed_at.0.to_rfc3339()),
            );
            nodes.push(HostProcessTopologyNode {
                id: process_node_id.clone(),
                object_kind: "ProcessRuntime",
                object_id: process.process_id.to_string(),
                layer: "resource",
                label: format!("{} ({})", basename(&process.executable), process.pid),
                properties: process_props,
            });
            let group_node_id = format!("process-group:{}:{}", host_node_id, process.executable);
            edges.push(HostProcessTopologyEdge {
                id: format!("edge:{}:{}", group_node_id, process.process_id),
                edge_kind: "host_process_assoc",
                source: group_node_id,
                target: process_node_id,
                label: None,
                properties: serde_json::Map::new(),
            });
        }

        for service_view in &view.services {
            let service_node_id = format!("service:{}", service_view.service.service_id);
            let mut service_props = serde_json::Map::new();
            service_props.insert(
                "serviceName".to_string(),
                Value::String(service_view.service.name.clone()),
            );
            if let Some(external_ref) = &service_view.service.external_ref {
                service_props.insert(
                    "externalRef".to_string(),
                    Value::String(external_ref.clone()),
                );
            }
            service_props.insert(
                "serviceType".to_string(),
                Value::String(format!("{:?}", service_view.service.service_type)),
            );
            service_props.insert(
                "boundary".to_string(),
                Value::String(format!("{:?}", service_view.service.boundary)),
            );
            nodes.push(HostProcessTopologyNode {
                id: service_node_id.clone(),
                object_kind: "ServiceEntity",
                object_id: service_view.service.service_id.to_string(),
                layer: "resource",
                label: service_view.service.name.clone(),
                properties: service_props,
            });
            edges.push(HostProcessTopologyEdge {
                id: format!("edge:{}:{}", host_node_id, service_view.service.service_id),
                edge_kind: "host_service_assoc",
                source: host_node_id.clone(),
                target: service_node_id.clone(),
                label: None,
                properties: serde_json::Map::new(),
            });

            for instance_view in &service_view.instances {
                let instance_node_id =
                    format!("service-instance:{}", instance_view.instance.instance_id);
                let mut instance_props = serde_json::Map::new();
                instance_props.insert(
                    "lastSeenAt".to_string(),
                    Value::String(instance_view.instance.last_seen_at.to_rfc3339()),
                );
                instance_props.insert(
                    "startedAt".to_string(),
                    Value::String(instance_view.instance.started_at.to_rfc3339()),
                );
                nodes.push(HostProcessTopologyNode {
                    id: instance_node_id.clone(),
                    object_kind: "ServiceInstance",
                    object_id: instance_view.instance.instance_id.to_string(),
                    layer: "resource",
                    label: format!(
                        "instance {}",
                        &instance_view.instance.instance_id.to_string()[..8]
                    ),
                    properties: instance_props,
                });
                edges.push(HostProcessTopologyEdge {
                    id: format!(
                        "edge:{}:{}",
                        instance_view.instance.instance_id, service_view.service.service_id
                    ),
                    edge_kind: "service_instance_assoc",
                    source: instance_node_id.clone(),
                    target: service_node_id.clone(),
                    label: None,
                    properties: serde_json::Map::new(),
                });

                for process in &instance_view.processes {
                    edges.push(HostProcessTopologyEdge {
                        id: format!(
                            "edge:{}:{}",
                            process.process_id, instance_view.instance.instance_id
                        ),
                        edge_kind: "process_service_assoc",
                        source: format!("process:{}", process.process_id),
                        target: instance_node_id.clone(),
                        label: None,
                        properties: serde_json::Map::new(),
                    });
                }
            }
        }
    }

    Ok(HostProcessTopologyGraph {
        metadata: HostProcessTopologyMetadata {
            query_time: Utc::now().to_rfc3339(),
            host_count: nodes
                .iter()
                .filter(|node| node.object_kind == "HostInventory")
                .count(),
            process_count,
        },
        nodes,
        edges,
    })
}

async fn build_host_process_topology_graph_async<S>(
    store: S,
    focus_host_id: Option<Uuid>,
) -> AppResult<HostProcessTopologyGraph>
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    let mut hosts = Vec::new();
    if let Some(host_id) = focus_host_id {
        let host = AsyncCatalogStore::get_host(&store, host_id)
            .await
            .conv_err()?
            .ok_or_else(|| materialization_missing(format!("host {host_id} was not found")))?;
        hosts.push(host);
    } else {
        let mut offset = 0;
        let page_limit = 200;
        loop {
            let page = Page {
                limit: page_limit,
                offset,
            };
            let batch = list_all_hosts_page_async(&store, page).await?;
            if batch.is_empty() {
                break;
            }
            offset += batch.len() as u32;
            hosts.extend(batch);
            if hosts.len() % page_limit as usize != 0 {
                break;
            }
        }
    }

    let query = TopologyQueryService::new(store.clone());
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut process_count = 0usize;

    for host in hosts {
        let Some(view) = query
            .host_topology_view_async(host.host_id)
            .await
            .conv_err()?
        else {
            continue;
        };

        let host_node_id = format!("host:{}", view.host.host_id);
        let mut host_props = serde_json::Map::new();
        host_props.insert(
            "hostName".to_string(),
            Value::String(view.host.host_name.clone()),
        );
        if let Some(machine_id) = &view.host.machine_id {
            host_props.insert("machineId".to_string(), Value::String(machine_id.clone()));
        }
        if let Some(os_name) = &view.host.os_name {
            host_props.insert("osName".to_string(), Value::String(os_name.clone()));
        }
        if let Some(os_version) = &view.host.os_version {
            host_props.insert("osVersion".to_string(), Value::String(os_version.clone()));
        }
        if let Some(runtime) = &view.latest_runtime {
            host_props.insert(
                "observedAt".to_string(),
                Value::String(runtime.observed_at.0.to_rfc3339()),
            );
            if let Some(loadavg) = runtime.loadavg_1m {
                host_props.insert("loadavg1m".to_string(), Value::from(loadavg));
            }
            if let Some(memory_used_bytes) = runtime.memory_used_bytes {
                host_props.insert(
                    "memoryUsedBytes".to_string(),
                    Value::from(memory_used_bytes),
                );
            }
            if let Some(processes) = runtime.process_count {
                host_props.insert("processCount".to_string(), Value::from(processes));
            }
        }
        nodes.push(HostProcessTopologyNode {
            id: host_node_id.clone(),
            object_kind: "HostInventory",
            object_id: view.host.host_id.to_string(),
            layer: "resource",
            label: view.host.host_name.clone(),
            properties: host_props,
        });

        let summary_node_id = format!("process-summary:{}", host_node_id);
        let mut summary_props = serde_json::Map::new();
        summary_props.insert(
            "totalProcesses".to_string(),
            Value::from(view.processes.len() as i64),
        );
        summary_props.insert(
            "totalPrograms".to_string(),
            Value::from(view.process_groups.len() as i64),
        );
        summary_props.insert(
            "topPrograms".to_string(),
            Value::String(
                view.process_groups
                    .iter()
                    .take(5)
                    .map(|group| format!("{} x{}", group.display_name, group.process_count))
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        );
        nodes.push(HostProcessTopologyNode {
            id: summary_node_id.clone(),
            object_kind: "ProcessSummary",
            object_id: view.host.host_id.to_string(),
            layer: "resource",
            label: format!("processes: {}", view.processes.len()),
            properties: summary_props,
        });
        edges.push(HostProcessTopologyEdge {
            id: format!("edge:{}:process-summary", view.host.host_id),
            edge_kind: "host_process_assoc",
            source: host_node_id.clone(),
            target: summary_node_id.clone(),
            label: None,
            properties: serde_json::Map::new(),
        });

        for group in &view.process_groups {
            let group_node_id = format!("process-group:{}:{}", host_node_id, group.executable);
            let mut group_props = serde_json::Map::new();
            group_props.insert(
                "executable".to_string(),
                Value::String(group.executable.clone()),
            );
            group_props.insert(
                "processCount".to_string(),
                Value::from(group.process_count as i64),
            );
            group_props.insert(
                "totalMemoryRssKiB".to_string(),
                Value::from(group.total_memory_rss_kib),
            );
            group_props.insert(
                "totalMemoryRssMiB".to_string(),
                Value::from(((group.total_memory_rss_kib as f64) / 1024.0 * 10.0).round() / 10.0),
            );
            if let Some(state) = &group.dominant_state {
                group_props.insert("dominantState".to_string(), Value::String(state.clone()));
            }
            group_props.insert(
                "states".to_string(),
                Value::String(
                    group
                        .state_summary
                        .iter()
                        .take(3)
                        .map(|item| format!("{}:{}", item.state, item.count))
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            );
            nodes.push(HostProcessTopologyNode {
                id: group_node_id.clone(),
                object_kind: "ProcessGroup",
                object_id: view.host.host_id.to_string(),
                layer: "resource",
                label: format!("{} x{}", group.display_name, group.process_count),
                properties: group_props,
            });
            edges.push(HostProcessTopologyEdge {
                id: format!("edge:{}:{}", summary_node_id, group.executable),
                edge_kind: "host_process_assoc",
                source: summary_node_id.clone(),
                target: group_node_id.clone(),
                label: None,
                properties: serde_json::Map::new(),
            });
        }

        for process in view.processes {
            process_count += 1;
            let process_node_id = format!("process:{}", process.process_id);
            let mut process_props = serde_json::Map::new();
            process_props.insert("pid".to_string(), Value::from(process.pid));
            process_props.insert(
                "executable".to_string(),
                Value::String(process.executable.clone()),
            );
            if let Some(command_line) = &process.command_line {
                process_props.insert(
                    "commandLine".to_string(),
                    Value::String(command_line.clone()),
                );
            }
            if let Some(state) = &process.process_state {
                process_props.insert("processState".to_string(), Value::String(state.clone()));
            }
            if let Some(memory_rss_kib) = process.memory_rss_kib {
                process_props.insert("memoryRssKiB".to_string(), Value::from(memory_rss_kib));
            }
            process_props.insert(
                "observedAt".to_string(),
                Value::String(process.observed_at.0.to_rfc3339()),
            );
            nodes.push(HostProcessTopologyNode {
                id: process_node_id.clone(),
                object_kind: "ProcessRuntime",
                object_id: process.process_id.to_string(),
                layer: "resource",
                label: format!("{} ({})", basename(&process.executable), process.pid),
                properties: process_props,
            });
            let group_node_id = format!("process-group:{}:{}", host_node_id, process.executable);
            edges.push(HostProcessTopologyEdge {
                id: format!("edge:{}:{}", group_node_id, process.process_id),
                edge_kind: "host_process_assoc",
                source: group_node_id,
                target: process_node_id,
                label: None,
                properties: serde_json::Map::new(),
            });
        }

        for service_view in &view.services {
            let service_node_id = format!("service:{}", service_view.service.service_id);
            let mut service_props = serde_json::Map::new();
            service_props.insert(
                "serviceName".to_string(),
                Value::String(service_view.service.name.clone()),
            );
            if let Some(external_ref) = &service_view.service.external_ref {
                service_props.insert(
                    "externalRef".to_string(),
                    Value::String(external_ref.clone()),
                );
            }
            service_props.insert(
                "serviceType".to_string(),
                Value::String(format!("{:?}", service_view.service.service_type)),
            );
            service_props.insert(
                "boundary".to_string(),
                Value::String(format!("{:?}", service_view.service.boundary)),
            );
            nodes.push(HostProcessTopologyNode {
                id: service_node_id.clone(),
                object_kind: "ServiceEntity",
                object_id: service_view.service.service_id.to_string(),
                layer: "resource",
                label: service_view.service.name.clone(),
                properties: service_props,
            });
            edges.push(HostProcessTopologyEdge {
                id: format!("edge:{}:{}", host_node_id, service_view.service.service_id),
                edge_kind: "host_service_assoc",
                source: host_node_id.clone(),
                target: service_node_id.clone(),
                label: None,
                properties: serde_json::Map::new(),
            });

            for instance_view in &service_view.instances {
                let instance_node_id =
                    format!("service-instance:{}", instance_view.instance.instance_id);
                let mut instance_props = serde_json::Map::new();
                instance_props.insert(
                    "lastSeenAt".to_string(),
                    Value::String(instance_view.instance.last_seen_at.to_rfc3339()),
                );
                instance_props.insert(
                    "startedAt".to_string(),
                    Value::String(instance_view.instance.started_at.to_rfc3339()),
                );
                nodes.push(HostProcessTopologyNode {
                    id: instance_node_id.clone(),
                    object_kind: "ServiceInstance",
                    object_id: instance_view.instance.instance_id.to_string(),
                    layer: "resource",
                    label: format!(
                        "instance {}",
                        &instance_view.instance.instance_id.to_string()[..8]
                    ),
                    properties: instance_props,
                });
                edges.push(HostProcessTopologyEdge {
                    id: format!(
                        "edge:{}:{}",
                        instance_view.instance.instance_id, service_view.service.service_id
                    ),
                    edge_kind: "service_instance_assoc",
                    source: instance_node_id.clone(),
                    target: service_node_id.clone(),
                    label: None,
                    properties: serde_json::Map::new(),
                });

                for process in &instance_view.processes {
                    edges.push(HostProcessTopologyEdge {
                        id: format!(
                            "edge:{}:{}",
                            process.process_id, instance_view.instance.instance_id
                        ),
                        edge_kind: "process_service_assoc",
                        source: format!("process:{}", process.process_id),
                        target: instance_node_id.clone(),
                        label: None,
                        properties: serde_json::Map::new(),
                    });
                }
            }
        }
    }

    Ok(HostProcessTopologyGraph {
        metadata: HostProcessTopologyMetadata {
            query_time: Utc::now().to_rfc3339(),
            host_count: nodes
                .iter()
                .filter(|node| node.object_kind == "HostInventory")
                .count(),
            process_count,
        },
        nodes,
        edges,
    })
}

fn basename(path: &str) -> String {
    path.rsplit('/')
        .next()
        .filter(|item| !item.is_empty())
        .unwrap_or(path)
        .to_string()
}

fn list_all_hosts_page<S>(store: &S, page: Page) -> AppResult<Vec<topology_domain::HostInventory>>
where
    S: CatalogStore,
{
    store.list_all_hosts(page).conv_err()
}

async fn list_all_hosts_page_async<S>(
    store: &S,
    page: Page,
) -> AppResult<Vec<topology_domain::HostInventory>>
where
    S: AsyncCatalogStore,
{
    AsyncCatalogStore::list_all_hosts(store, page)
        .await
        .conv_err()
}

pub fn parse_monolith_input(args: &[String]) -> AppResult<(MonolithMode, MonolithInput)> {
    match args {
        [] => Ok((MonolithMode::Memory, MonolithInput::Demo)),
        [cmd] if cmd == "demo" => Ok((MonolithMode::Memory, MonolithInput::Demo)),
        [cmd, path] if cmd == "file" => Ok((
            MonolithMode::Memory,
            MonolithInput::File(PathBuf::from(path)),
        )),
        [cmd] if cmd == "postgres-mock" => Ok((MonolithMode::PostgresMock, MonolithInput::Demo)),
        [mode, cmd, path] if mode == "postgres-mock" && cmd == "file" => Ok((
            MonolithMode::PostgresMock,
            MonolithInput::File(PathBuf::from(path)),
        )),
        [cmd] if cmd == "postgres-live" => Ok((MonolithMode::PostgresLive, MonolithInput::Demo)),
        [mode, cmd, path] if mode == "postgres-live" && cmd == "file" => Ok((
            MonolithMode::PostgresLive,
            MonolithInput::File(PathBuf::from(path)),
        )),
        [mode, cmd] if mode == "postgres-mock" && cmd == "reset-public" => {
            Ok((MonolithMode::PostgresMock, MonolithInput::ResetPublic))
        }
        [mode, cmd] if mode == "postgres-live" && cmd == "reset-public" => {
            Ok((MonolithMode::PostgresLive, MonolithInput::ResetPublic))
        }
        [mode, cmd, path] if mode == "postgres-mock" && cmd == "export-visualization" => Ok((
            MonolithMode::PostgresMock,
            MonolithInput::ExportVisualization(PathBuf::from(path)),
        )),
        [mode, cmd, path] if mode == "postgres-live" && cmd == "export-visualization" => Ok((
            MonolithMode::PostgresLive,
            MonolithInput::ExportVisualization(PathBuf::from(path)),
        )),
        [mode, cmd] if mode == "postgres-live" && cmd == "print-first-host-process-topology" => {
            Ok((
                MonolithMode::PostgresLive,
                MonolithInput::PrintFirstHostProcessTopology,
            ))
        }
        [mode, cmd, host_id] if mode == "postgres-live" && cmd == "print-host-process-topology" => {
            Ok((
                MonolithMode::PostgresLive,
                MonolithInput::PrintHostProcessTopology(parse_uuid_arg(host_id)?),
            ))
        }
        [mode, cmd] if mode == "postgres-mock" && cmd == "print-first-host-process-topology" => {
            Ok((
                MonolithMode::PostgresMock,
                MonolithInput::PrintFirstHostProcessTopology,
            ))
        }
        [mode, cmd, host_id] if mode == "postgres-mock" && cmd == "print-host-process-topology" => {
            Ok((
                MonolithMode::PostgresMock,
                MonolithInput::PrintHostProcessTopology(parse_uuid_arg(host_id)?),
            ))
        }
        [mode, cmd, flag, listen]
            if (mode == "postgres-live" || mode == "postgres-mock" || mode == "memory")
                && cmd == "serve"
                && flag == "--listen" =>
        {
            let mode = match mode.as_str() {
                "postgres-live" => MonolithMode::PostgresLive,
                "postgres-mock" => MonolithMode::PostgresMock,
                "memory" => MonolithMode::Memory,
                _ => return Err(invalid_args()),
            };
            Ok((
                mode,
                MonolithInput::Serve {
                    listen: listen.clone(),
                },
            ))
        }
        [cmd, paths @ ..] if cmd == "replay-jsonl" && !paths.is_empty() => Ok((
            MonolithMode::Memory,
            MonolithInput::JsonlFiles(paths.iter().map(PathBuf::from).collect()),
        )),
        [cmd, paths @ ..] if cmd == "import-jsonl" && !paths.is_empty() => Ok((
            MonolithMode::Memory,
            MonolithInput::JsonlFiles(paths.iter().map(PathBuf::from).collect()),
        )),
        [mode, cmd, paths @ ..]
            if mode == "postgres-mock" && cmd == "replay-jsonl" && !paths.is_empty() =>
        {
            Ok((
                MonolithMode::PostgresMock,
                MonolithInput::JsonlFiles(paths.iter().map(PathBuf::from).collect()),
            ))
        }
        [mode, cmd, paths @ ..]
            if mode == "postgres-mock" && cmd == "import-jsonl" && !paths.is_empty() =>
        {
            Ok((
                MonolithMode::PostgresMock,
                MonolithInput::JsonlFiles(paths.iter().map(PathBuf::from).collect()),
            ))
        }
        [mode, cmd, paths @ ..]
            if mode == "postgres-mock" && cmd == "replace-jsonl" && !paths.is_empty() =>
        {
            Ok((
                MonolithMode::PostgresMock,
                MonolithInput::ReplaceJsonlFiles(paths.iter().map(PathBuf::from).collect()),
            ))
        }
        [mode, cmd, paths @ ..]
            if mode == "postgres-live" && cmd == "replay-jsonl" && !paths.is_empty() =>
        {
            Ok((
                MonolithMode::PostgresLive,
                MonolithInput::JsonlFiles(paths.iter().map(PathBuf::from).collect()),
            ))
        }
        [mode, cmd, paths @ ..]
            if mode == "postgres-live" && cmd == "import-jsonl" && !paths.is_empty() =>
        {
            Ok((
                MonolithMode::PostgresLive,
                MonolithInput::JsonlFiles(paths.iter().map(PathBuf::from).collect()),
            ))
        }
        [mode, cmd, paths @ ..]
            if mode == "postgres-live" && cmd == "replace-jsonl" && !paths.is_empty() =>
        {
            Ok((
                MonolithMode::PostgresLive,
                MonolithInput::ReplaceJsonlFiles(paths.iter().map(PathBuf::from).collect()),
            ))
        }
        _ => Err(invalid_args()),
    }
}

fn resolve_database_url() -> String {
    std::env::var("DATABASE_URL")
        .or_else(|_| std::env::var("DAYU_TOPOLOGY_DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://dayu:dayu@127.0.0.1:55432/dayu_topology".to_string())
}

fn parse_uuid_arg(value: &str) -> AppResult<Uuid> {
    Uuid::parse_str(value).map_err(|err| {
        AppReason::InvalidArgs
            .to_err()
            .with_detail(format!("parse uuid argument {value}: {err}"))
    })
}

async fn healthz() -> &'static str {
    "ok"
}

async fn get_host_process_topology(
    State(state): State<HttpAppState>,
    AxumPath(host_id): AxumPath<String>,
) -> Response {
    let host_id = match parse_uuid_arg(&host_id) {
        Ok(host_id) => host_id,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, err.to_string()),
    };

    match &state.app {
        TopologyMonolith::Memory(store) => {
            host_process_topology_response(store.clone(), Some(host_id)).await
        }
        TopologyMonolith::PostgresMock(store) => {
            host_process_topology_response(store.clone(), Some(host_id)).await
        }
        TopologyMonolith::PostgresLive(store) => {
            host_process_topology_response(store.clone(), Some(host_id)).await
        }
    }
}

async fn get_first_host_process_topology(State(state): State<HttpAppState>) -> Response {
    match &state.app {
        TopologyMonolith::Memory(store) => {
            host_process_topology_response(store.clone(), None).await
        }
        TopologyMonolith::PostgresMock(store) => {
            host_process_topology_response(store.clone(), None).await
        }
        TopologyMonolith::PostgresLive(store) => {
            host_process_topology_response(store.clone(), None).await
        }
    }
}

async fn get_host_topology(
    State(state): State<HttpAppState>,
    AxumPath(host_id): AxumPath<String>,
) -> Response {
    let host_id = match parse_uuid_arg(&host_id) {
        Ok(host_id) => host_id,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, err.to_string()),
    };

    match &state.app {
        TopologyMonolith::Memory(store) => host_topology_response(store.clone(), host_id).await,
        TopologyMonolith::PostgresMock(store) => {
            host_topology_response(store.clone(), host_id).await
        }
        TopologyMonolith::PostgresLive(store) => {
            host_topology_response(store.clone(), host_id).await
        }
    }
}

async fn get_first_host_topology(State(state): State<HttpAppState>) -> Response {
    match &state.app {
        TopologyMonolith::Memory(store) => first_host_topology_response(store.clone()).await,
        TopologyMonolith::PostgresMock(store) => first_host_topology_response(store.clone()).await,
        TopologyMonolith::PostgresLive(store) => first_host_topology_response(store.clone()).await,
    }
}

async fn get_host_process_overview(
    State(state): State<HttpAppState>,
    AxumPath(host_id): AxumPath<String>,
    Query(query): Query<HostProcessOverviewQuery>,
) -> Response {
    let host_id = match parse_uuid_arg(&host_id) {
        Ok(host_id) => host_id,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, err.to_string()),
    };
    let top_n = query.top_n.unwrap_or(12).clamp(1, 200);

    match &state.app {
        TopologyMonolith::Memory(store) => {
            host_process_overview_response(store.clone(), host_id, top_n).await
        }
        TopologyMonolith::PostgresMock(store) => {
            host_process_overview_response(store.clone(), host_id, top_n).await
        }
        TopologyMonolith::PostgresLive(store) => {
            host_process_overview_response(store.clone(), host_id, top_n).await
        }
    }
}

async fn get_host_process_groups_page(
    State(state): State<HttpAppState>,
    AxumPath(host_id): AxumPath<String>,
    Query(query): Query<HostProcessGroupsQuery>,
) -> Response {
    let host_id = match parse_uuid_arg(&host_id) {
        Ok(host_id) => host_id,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, err.to_string()),
    };
    let limit = query.limit.unwrap_or(50).clamp(1, 500);
    let offset = query.offset.unwrap_or(0);

    match &state.app {
        TopologyMonolith::Memory(store) => {
            host_process_groups_page_response(store.clone(), host_id, offset, limit).await
        }
        TopologyMonolith::PostgresMock(store) => {
            host_process_groups_page_response(store.clone(), host_id, offset, limit).await
        }
        TopologyMonolith::PostgresLive(store) => {
            host_process_groups_page_response(store.clone(), host_id, offset, limit).await
        }
    }
}

async fn host_topology_response<S>(store: S, host_id: Uuid) -> Response
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    let query = TopologyQueryService::new(store);
    match query.host_topology_view_async(host_id).await {
        Ok(Some(view)) => {
            let payload = HostTopologyHttpEnvelope {
                status: "ok",
                data: HostTopologyHttpView {
                    host: HostTopologyHostDto {
                        id: view.host.host_id,
                        host_name: view.host.host_name,
                        machine_id: view.host.machine_id,
                        os_name: view.host.os_name,
                        os_version: view.host.os_version,
                    },
                    latest_runtime: view.latest_runtime,
                    process_groups: view.process_groups,
                    processes: view.processes,
                    network_segments: view.network_segments,
                    network_assocs: view.network_assocs,
                    services: view.services,
                    assignments: view.assignments,
                    generated_at: view.generated_at,
                },
            };
            (StatusCode::OK, Json(payload)).into_response()
        }
        Ok(None) => json_error(StatusCode::NOT_FOUND, format!("host {host_id} not found")),
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn first_host_topology_response<S>(store: S) -> Response
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    let hosts = match AsyncCatalogStore::list_all_hosts(&store, Page::default()).await {
        Ok(hosts) => hosts,
        Err(err) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    };
    let Some(host) = hosts.into_iter().next() else {
        return json_error(StatusCode::NOT_FOUND, "no host found".to_string());
    };
    host_topology_response(store, host.host_id).await
}

async fn host_process_overview_response<S>(store: S, host_id: Uuid, top_n: usize) -> Response
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    let query = TopologyQueryService::new(store);
    match query.host_process_overview_view_async(host_id, top_n).await {
        Ok(Some(view)) => {
            let payload = HostProcessOverviewHttpEnvelope {
                status: "ok",
                data: HostProcessOverviewHttpView {
                    host: HostTopologyHostDto {
                        id: view.host.host_id,
                        host_name: view.host.host_name,
                        machine_id: view.host.machine_id,
                        os_name: view.host.os_name,
                        os_version: view.host.os_version,
                    },
                    total_processes: view.total_processes,
                    total_groups: view.total_groups,
                    top_groups: view.top_groups,
                    truncated_group_count: view.truncated_group_count,
                    generated_at: view.generated_at,
                },
            };
            (StatusCode::OK, Json(payload)).into_response()
        }
        Ok(None) => json_error(StatusCode::NOT_FOUND, format!("host {host_id} not found")),
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn host_process_groups_page_response<S>(
    store: S,
    host_id: Uuid,
    offset: usize,
    limit: usize,
) -> Response
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    let query = TopologyQueryService::new(store);
    match query
        .host_process_groups_page_view_async(host_id, offset, limit)
        .await
    {
        Ok(Some(view)) => {
            let payload = HostProcessGroupsPageHttpEnvelope {
                status: "ok",
                data: HostProcessGroupsPageHttpView {
                    host: HostTopologyHostDto {
                        id: view.host.host_id,
                        host_name: view.host.host_name,
                        machine_id: view.host.machine_id,
                        os_name: view.host.os_name,
                        os_version: view.host.os_version,
                    },
                    total_processes: view.total_processes,
                    total_groups: view.total_groups,
                    groups: view.groups,
                    limit: view.limit,
                    offset: view.offset,
                    has_more: view.has_more,
                    generated_at: view.generated_at,
                },
            };
            (StatusCode::OK, Json(payload)).into_response()
        }
        Ok(None) => json_error(StatusCode::NOT_FOUND, format!("host {host_id} not found")),
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn host_process_topology_response<S>(store: S, focus_host_id: Option<Uuid>) -> Response
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    match build_host_process_topology_graph_async(store, focus_host_id).await {
        Ok(graph) => {
            let payload = VisualizationEnvelope {
                status: "ok",
                data: graph,
            };
            (StatusCode::OK, Json(payload)).into_response()
        }
        Err(err) => json_error(StatusCode::NOT_FOUND, err.to_string()),
    }
}

fn json_error(status: StatusCode, message: String) -> Response {
    (
        status,
        Json(serde_json::json!({
            "status": "error",
            "code": status.as_u16().to_string(),
            "message": message,
        })),
    )
        .into_response()
}

fn build_http_router(state: HttpAppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/topology/host/first", get(get_first_host_topology))
        .route("/api/topology/host/{id}", get(get_host_topology))
        .route(
            "/api/topology/host/{id}/process-overview",
            get(get_host_process_overview),
        )
        .route(
            "/api/topology/host/{id}/process-groups",
            get(get_host_process_groups_page),
        )
        .route(
            "/api/topology/host/first/processes",
            get(get_first_host_process_topology),
        )
        .route(
            "/api/topology/host/{id}/processes",
            get(get_host_process_topology),
        )
        .with_state(state)
}

fn load_demo_payload() -> AppResult<Value> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/p0_monolith_demo.json");
    load_payload_from_file(path)
}

fn load_payload_from_file(path: impl AsRef<Path>) -> AppResult<Value> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path).source_err(
        AppReason::InputLoadFailed,
        format!("read {}", path.display()),
    )?;
    serde_json::from_str(&raw).source_err(
        AppReason::InputLoadFailed,
        format!("parse {} as json", path.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::util::ServiceExt;

    #[test]
    fn monolith_demo_runs_end_to_end() {
        let app = TopologyMonolith::new_in_memory();
        let summary = app.run_demo().unwrap();

        assert_eq!(summary.ingest_id, "demo-ingest-1");
        assert_eq!(summary.host_name, "demo-node");
        assert_eq!(summary.network_name.as_deref(), Some("10.42.0.0/24"));
        assert_eq!(summary.assoc_ip.as_deref(), Some("10.42.0.12"));
        assert_eq!(summary.responsibilities, vec!["alice:Owner".to_string()]);
    }

    #[test]
    fn parse_monolith_input_defaults_to_demo() {
        let (mode, input) = parse_monolith_input(&[]).unwrap();
        assert_eq!(mode, MonolithMode::Memory);
        assert!(matches!(input, MonolithInput::Demo));
    }

    #[test]
    fn parse_monolith_input_supports_file_mode() {
        let (mode, input) =
            parse_monolith_input(&["file".to_string(), "/tmp/demo.json".to_string()]).unwrap();
        assert_eq!(mode, MonolithMode::Memory);
        match input {
            MonolithInput::File(path) => assert_eq!(path, PathBuf::from("/tmp/demo.json")),
            _ => panic!("expected file mode"),
        }
    }

    #[test]
    fn parse_monolith_input_supports_replay_jsonl_mode() {
        let (mode, input) = parse_monolith_input(&[
            "replay-jsonl".to_string(),
            "/tmp/dayu-edge.jsonl".to_string(),
        ])
        .unwrap();
        assert_eq!(mode, MonolithMode::Memory);
        match input {
            MonolithInput::JsonlFiles(paths) => {
                assert_eq!(paths, vec![PathBuf::from("/tmp/dayu-edge.jsonl")])
            }
            _ => panic!("expected replay-jsonl mode"),
        }
    }

    #[test]
    fn parse_monolith_input_supports_replay_jsonl_multi_file_mode() {
        let (mode, input) = parse_monolith_input(&[
            "replay-jsonl".to_string(),
            "/tmp/dayu-edge.jsonl".to_string(),
            "/tmp/dayu-telemetry.jsonl".to_string(),
        ])
        .unwrap();
        assert_eq!(mode, MonolithMode::Memory);
        match input {
            MonolithInput::JsonlFiles(paths) => assert_eq!(
                paths,
                vec![
                    PathBuf::from("/tmp/dayu-edge.jsonl"),
                    PathBuf::from("/tmp/dayu-telemetry.jsonl")
                ]
            ),
            _ => panic!("expected replay-jsonl multi-file mode"),
        }
    }

    #[test]
    fn parse_monolith_input_supports_replay_jsonl_many_files() {
        let (mode, input) = parse_monolith_input(&[
            "replay-jsonl".to_string(),
            "/tmp/a.jsonl".to_string(),
            "/tmp/b.jsonl".to_string(),
            "/tmp/c.jsonl".to_string(),
        ])
        .unwrap();
        assert_eq!(mode, MonolithMode::Memory);
        match input {
            MonolithInput::JsonlFiles(paths) => assert_eq!(
                paths,
                vec![
                    PathBuf::from("/tmp/a.jsonl"),
                    PathBuf::from("/tmp/b.jsonl"),
                    PathBuf::from("/tmp/c.jsonl")
                ]
            ),
            _ => panic!("expected replay-jsonl many-file mode"),
        }
    }

    #[test]
    fn parse_monolith_input_supports_postgres_mock_mode() {
        let (mode, input) = parse_monolith_input(&["postgres-mock".to_string()]).unwrap();
        assert_eq!(mode, MonolithMode::PostgresMock);
        assert!(matches!(input, MonolithInput::Demo));
    }

    #[test]
    fn parse_monolith_input_supports_postgres_live_mode() {
        let (mode, input) = parse_monolith_input(&["postgres-live".to_string()]).unwrap();
        assert_eq!(mode, MonolithMode::PostgresLive);
        assert!(matches!(input, MonolithInput::Demo));
    }

    #[test]
    fn parse_monolith_input_supports_postgres_live_reset_public_mode() {
        let (mode, input) =
            parse_monolith_input(&["postgres-live".to_string(), "reset-public".to_string()])
                .unwrap();
        assert_eq!(mode, MonolithMode::PostgresLive);
        assert!(matches!(input, MonolithInput::ResetPublic));
    }

    #[test]
    fn parse_monolith_input_supports_postgres_live_export_visualization_mode() {
        let (mode, input) = parse_monolith_input(&[
            "postgres-live".to_string(),
            "export-visualization".to_string(),
            "/tmp/live-topology.json".to_string(),
        ])
        .unwrap();
        assert_eq!(mode, MonolithMode::PostgresLive);
        match input {
            MonolithInput::ExportVisualization(path) => {
                assert_eq!(path, PathBuf::from("/tmp/live-topology.json"))
            }
            _ => panic!("expected export-visualization mode"),
        }
    }

    #[test]
    fn parse_monolith_input_supports_postgres_live_print_first_host_process_topology_mode() {
        let (mode, input) = parse_monolith_input(&[
            "postgres-live".to_string(),
            "print-first-host-process-topology".to_string(),
        ])
        .unwrap();
        assert_eq!(mode, MonolithMode::PostgresLive);
        assert!(matches!(
            input,
            MonolithInput::PrintFirstHostProcessTopology
        ));
    }

    #[test]
    fn parse_monolith_input_supports_postgres_live_import_jsonl_mode() {
        let (mode, input) = parse_monolith_input(&[
            "postgres-live".to_string(),
            "import-jsonl".to_string(),
            "/tmp/dayu-edge.jsonl".to_string(),
        ])
        .unwrap();
        assert_eq!(mode, MonolithMode::PostgresLive);
        match input {
            MonolithInput::JsonlFiles(paths) => {
                assert_eq!(paths, vec![PathBuf::from("/tmp/dayu-edge.jsonl")])
            }
            _ => panic!("expected import-jsonl mode"),
        }
    }

    #[test]
    fn parse_monolith_input_supports_postgres_live_replace_jsonl_mode() {
        let (mode, input) = parse_monolith_input(&[
            "postgres-live".to_string(),
            "replace-jsonl".to_string(),
            "/tmp/dayu-edge.jsonl".to_string(),
            "/tmp/dayu-telemetry.jsonl".to_string(),
        ])
        .unwrap();
        assert_eq!(mode, MonolithMode::PostgresLive);
        match input {
            MonolithInput::ReplaceJsonlFiles(paths) => assert_eq!(
                paths,
                vec![
                    PathBuf::from("/tmp/dayu-edge.jsonl"),
                    PathBuf::from("/tmp/dayu-telemetry.jsonl")
                ]
            ),
            _ => panic!("expected replace-jsonl mode"),
        }
    }

    #[test]
    fn postgres_mock_demo_runs_end_to_end() {
        let app = TopologyAppBuilder::new()
            .with_mode(MonolithMode::PostgresMock)
            .build()
            .unwrap();
        let summary = app.run_demo().unwrap();

        assert_eq!(summary.ingest_id, "demo-ingest-1");
        assert_eq!(summary.host_name, "demo-node");
        assert_eq!(summary.network_name.as_deref(), Some("10.42.0.0/24"));
        assert_eq!(summary.assoc_ip.as_deref(), Some("10.42.0.12"));
        assert_eq!(summary.responsibilities, vec!["alice:Owner".to_string()]);
    }

    #[test]
    fn file_mode_accepts_target_dayu_input_envelope() {
        let app = TopologyMonolith::new_in_memory();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/external-input/target/edge-discovery-snapshot.json");

        let summary = app.run_file(fixture).unwrap();

        assert_eq!(
            summary.ingest_id,
            "dayu.in.edge.v1:warp-insight:agent-office-01:tenant-demo:office:edge-snap-20260426-office-01"
        );
        assert_eq!(summary.host_name, "office-build-01");
        assert_eq!(summary.network_name.as_deref(), Some("192.168.10.0/24"));
        assert_eq!(summary.assoc_ip.as_deref(), Some("192.168.10.52"));
    }

    #[test]
    fn file_mode_accepts_dayu_edge_host_only_payload() {
        let app = TopologyMonolith::new_in_memory();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/dayu_edge_host_only.json");

        let summary = app.run_file(fixture).unwrap();

        assert_eq!(
            summary.ingest_id,
            "dayu.in.edge.v1:warp-insight:agent-local-01:tenant-demo:office:edge-snap-local-01"
        );
        assert_eq!(summary.host_name, "local-host");
        assert_eq!(summary.network_name, None);
        assert_eq!(summary.assoc_ip, None);
    }

    #[test]
    fn replay_jsonl_mode_materializes_host_and_processes() {
        let app = TopologyMonolith::new_in_memory();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/dayu_edge_host_process_sample.jsonl");

        let summary = app.replay_jsonl(fixture).unwrap();

        assert_eq!(summary.total_lines, 2);
        assert_eq!(summary.success_lines, 2);
        assert_eq!(summary.failed_lines, 0);
        assert_eq!(summary.host_count, 1);
        assert_eq!(summary.network_count, 0);
        assert_eq!(summary.process_count, 1);
        assert_eq!(summary.enriched_process_count, 0);
        assert_eq!(summary.host_runtime_count, 0);
        assert!(summary.last_ingest_id.is_some());
        assert!(summary.failures.is_empty());
    }

    #[test]
    fn print_first_host_process_topology_outputs_specialized_graph_json() {
        let app = TopologyMonolith::new_postgres_mock().unwrap();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/dayu_edge_host_process_sample.jsonl");
        app.replay_jsonl(fixture).unwrap();

        let result = app
            .run(MonolithInput::PrintFirstHostProcessTopology)
            .unwrap();
        let body = match result {
            MonolithRunResult::PrintJson(body) => body,
            other => panic!(
                "expected print json result, got {:?}",
                std::mem::discriminant(&other)
            ),
        };

        assert!(body.contains("\"objectKind\": \"HostInventory\""));
        assert!(body.contains("\"objectKind\": \"ProcessSummary\""));
        assert!(body.contains("\"objectKind\": \"ProcessGroup\""));
        assert!(body.contains("\"objectKind\": \"ProcessRuntime\""));
    }

    #[tokio::test]
    async fn http_router_returns_first_host_process_topology_in_memory() {
        let app = TopologyMonolith::new_in_memory();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/dayu_edge_host_process_sample.jsonl");
        app.replay_jsonl(fixture).unwrap();

        let router = build_http_router(HttpAppState { app });
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/topology/host/first/processes")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn http_router_returns_structured_host_topology_with_services_and_processes() {
        let app = TopologyMonolith::new_in_memory();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/dayu_edge_service_binding_sample.jsonl");
        app.replay_jsonl(fixture).unwrap();

        let host_id = app
            .store()
            .and_then(|store| CatalogStore::list_all_hosts(store, Page::default()).ok())
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .host_id;

        let router = build_http_router(HttpAppState { app });
        let response = router
            .oneshot(
                Request::builder()
                    .uri(format!("/api/topology/host/{host_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["status"], "ok");
        assert_eq!(payload["data"]["host"]["hostName"], "local-host");
        assert_eq!(
            payload["data"]["processGroups"].as_array().unwrap().len(),
            1
        );
        assert_eq!(payload["data"]["processes"].as_array().unwrap().len(), 1);
        assert_eq!(payload["data"]["services"].as_array().unwrap().len(), 1);
        assert_eq!(payload["data"]["services"][0]["service"]["name"], "sshd");
        assert_eq!(
            payload["data"]["services"][0]["instances"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            payload["data"]["services"][0]["instances"][0]["processes"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn http_router_returns_first_host_topology() {
        let app = TopologyMonolith::new_in_memory();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/dayu_edge_service_binding_sample.jsonl");
        app.replay_jsonl(fixture).unwrap();

        let router = build_http_router(HttpAppState { app });
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/topology/host/first")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["status"], "ok");
        assert_eq!(payload["data"]["host"]["hostName"], "local-host");
        assert_eq!(payload["data"]["services"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn http_router_returns_host_process_overview() {
        let app = TopologyMonolith::new_in_memory();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/dayu_edge_host_process_sample.jsonl");
        app.replay_jsonl(fixture).unwrap();

        let host_id = app
            .store()
            .and_then(|store| CatalogStore::list_all_hosts(store, Page::default()).ok())
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .host_id;

        let router = build_http_router(HttpAppState { app });
        let response = router
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/topology/host/{host_id}/process-overview?top_n=5"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["status"], "ok");
        assert_eq!(payload["data"]["host"]["hostName"], "local-host");
        assert_eq!(payload["data"]["totalProcesses"], 1);
        assert_eq!(payload["data"]["totalGroups"], 1);
        assert_eq!(payload["data"]["topGroups"].as_array().unwrap().len(), 1);
        assert_eq!(payload["data"]["truncatedGroupCount"], 0);
    }

    #[tokio::test]
    async fn http_router_returns_host_process_groups_page() {
        let app = TopologyMonolith::new_in_memory();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/dayu_edge_host_process_sample.jsonl");
        app.replay_jsonl(fixture).unwrap();

        let host_id = app
            .store()
            .and_then(|store| CatalogStore::list_all_hosts(store, Page::default()).ok())
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .host_id;

        let router = build_http_router(HttpAppState { app });
        let response = router
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/topology/host/{host_id}/process-groups?limit=10&offset=0"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["status"], "ok");
        assert_eq!(payload["data"]["host"]["hostName"], "local-host");
        assert_eq!(payload["data"]["totalProcesses"], 1);
        assert_eq!(payload["data"]["totalGroups"], 1);
        assert_eq!(payload["data"]["groups"].as_array().unwrap().len(), 1);
        assert_eq!(payload["data"]["limit"], 10);
        assert_eq!(payload["data"]["offset"], 0);
        assert_eq!(payload["data"]["hasMore"], false);
    }
}
