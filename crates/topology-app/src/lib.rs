pub mod error;

use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use orion_error::{conversion::ConvErr, prelude::*};
use serde_json::Value;
use topology_api::{TopologyIngestService, TopologyQueryService};
use topology_domain::{
    DayuInputEnvelope, IngestEnvelope, IngestMode, ObjectKind, SourceKind, TenantId,
};
use topology_storage::{
    CatalogStore, GovernanceStore, InMemoryTopologyStore, IngestStore, MemoryPostgresExecutor,
    Page, PostgresTopologyStore, RuntimeStore,
};
use uuid::Uuid;

pub use error::{AppError, AppReason, AppResult};
use error::{invalid_args, materialization_missing};

pub enum TopologyMonolith {
    Memory(InMemoryTopologyStore),
    PostgresMock(PostgresTopologyStore<MemoryPostgresExecutor>),
}

pub struct TopologyAppBuilder {
    mode: MonolithMode,
}

pub struct MonolithRunSummary {
    pub ingest_id: String,
    pub host_name: String,
    pub network_name: String,
    pub assoc_ip: String,
    pub responsibilities: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonolithMode {
    Memory,
    PostgresMock,
}

pub enum MonolithInput {
    Demo,
    File(PathBuf),
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

    pub fn run(&self, input: MonolithInput) -> AppResult<MonolithRunSummary> {
        match self {
            Self::Memory(store) => run_with_store(store.clone(), input),
            Self::PostgresMock(store) => run_with_store(store.clone(), input),
        }
    }

    pub fn run_demo(&self) -> AppResult<MonolithRunSummary> {
        self.run(MonolithInput::Demo)
    }

    pub fn run_file(&self, path: impl AsRef<Path>) -> AppResult<MonolithRunSummary> {
        self.run(MonolithInput::File(path.as_ref().to_path_buf()))
    }

    pub fn store(&self) -> Option<&InMemoryTopologyStore> {
        match self {
            Self::Memory(store) => Some(store),
            Self::PostgresMock(_) => None,
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
        }
    }
}

impl Default for TopologyAppBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn run_with_store<S>(store: S, input: MonolithInput) -> AppResult<MonolithRunSummary>
where
    S: Clone + CatalogStore + RuntimeStore + GovernanceStore + IngestStore,
{
    let tenant_id = TenantId(Uuid::new_v4());
    let payload_inline = match input {
        MonolithInput::Demo => load_demo_payload()?,
        MonolithInput::File(path) => load_payload_from_file(&path)?,
    };
    run_with_payload(store, tenant_id, payload_inline)
}

fn run_with_payload<S>(
    store: S,
    tenant_id: TenantId,
    payload_inline: Value,
) -> AppResult<MonolithRunSummary>
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
    let assoc = host_view
        .network_assocs
        .first()
        .ok_or_else(|| materialization_missing("host network association was not built"))?;
    let network = host_view
        .network_segments
        .first()
        .ok_or_else(|| materialization_missing("network topology segment was not built"))?;
    let responsibility_views = query
        .effective_responsibility_view(ObjectKind::Host, host.host_id)
        .conv_err()?;

    Ok(MonolithRunSummary {
        ingest_id: record.ingest_id,
        host_name: host_view.host.host_name,
        network_name: network.name.clone(),
        assoc_ip: assoc.ip_addr.clone(),
        responsibilities: responsibility_views
            .into_iter()
            .map(|view| format!("{}:{:?}", view.subject.display_name, view.assignment.role))
            .collect(),
    })
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

pub fn parse_monolith_input(args: &[String]) -> AppResult<(MonolithMode, MonolithInput)> {
    match args {
        [] => Ok((MonolithMode::Memory, MonolithInput::Demo)),
        [cmd] if cmd == "demo" => Ok((MonolithMode::Memory, MonolithInput::Demo)),
        [cmd, path] if cmd == "file" => Ok((
            MonolithMode::Memory,
            MonolithInput::File(PathBuf::from(path)),
        )),
        [cmd] if cmd == "postgres-mock" => Ok((MonolithMode::PostgresMock, MonolithInput::Demo)),
        _ => Err(invalid_args()),
    }
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

    #[test]
    fn monolith_demo_runs_end_to_end() {
        let app = TopologyMonolith::new_in_memory();
        let summary = app.run_demo().unwrap();

        assert_eq!(summary.ingest_id, "demo-ingest-1");
        assert_eq!(summary.host_name, "demo-node");
        assert_eq!(summary.network_name, "10.42.0.0/24");
        assert_eq!(summary.assoc_ip, "10.42.0.12");
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
            MonolithInput::Demo => panic!("expected file mode"),
        }
    }

    #[test]
    fn parse_monolith_input_supports_postgres_mock_mode() {
        let (mode, input) = parse_monolith_input(&["postgres-mock".to_string()]).unwrap();
        assert_eq!(mode, MonolithMode::PostgresMock);
        assert!(matches!(input, MonolithInput::Demo));
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
        assert_eq!(summary.network_name, "10.42.0.0/24");
        assert_eq!(summary.assoc_ip, "10.42.0.12");
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
        assert_eq!(summary.network_name, "192.168.10.0/24");
        assert_eq!(summary.assoc_ip, "192.168.10.52");
    }
}
