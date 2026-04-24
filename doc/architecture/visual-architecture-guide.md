# dayu-topology 可视化架构指南

## 1. 文档目的

本文档把 `dayu-topology` 的关键设计、结构和过程整理成可传播的专业图示。

目标是帮助读者快速理解：

- 系统边界是什么
- 核心模块如何协作
- 数据如何进入中心模型
- source of truth 与 read model 如何分层
- 外部同步、identity resolution、查询和部署如何落地

相关文档：

- [`project-charter.md`](./project-charter.md)
- [`system-architecture.md`](./system-architecture.md)
- [`dataflow-and-pipeline-architecture.md`](./dataflow-and-pipeline-architecture.md)
- [`identity-resolution-architecture.md`](./identity-resolution-architecture.md)
- [`external-sync-architecture.md`](./external-sync-architecture.md)
- [`query-and-read-model-architecture.md`](./query-and-read-model-architecture.md)
- [`unified-model-overview.md`](./unified-model-overview.md)
- [`../roadmap/development-plan.md`](../roadmap/development-plan.md)
- [`../roadmap/todo-backlog.md`](../roadmap/todo-backlog.md)

---

## 2. 一页总览

```mermaid
flowchart LR
    subgraph Producers["事实来源"]
        Edge["Edge / Discovery<br/>host / process / pod / runtime"]
        Manual["Manual / Batch<br/>业务目录 / 责任关系"]
        Telemetry["Telemetry Summary<br/>trace / log / flow 摘要"]
        External["External Systems<br/>CMDB / LDAP / IAM / Oncall / Vuln"]
    end

    subgraph Topology["dayu-topology 中心侧"]
        Ingest["Ingest Gateway<br/>鉴权 / 幂等 / envelope"]
        Normalize["Normalization Engine<br/>candidate / resolver / materializer"]
        Store[("Catalog Store<br/>PostgreSQL source of truth")]
        ObjectStore[("Object Storage<br/>raw payload / snapshot")]
        Derive["Derived View Builder<br/>summary / graph / explain view"]
        Query["Query API<br/>catalog / topology / governance / explain"]
        Sync["External Sync Workers<br/>connector / cursor / staged payload"]
    end

    subgraph Consumers["消费方"]
        UI["控制中心 / UI"]
        Analytics["分析系统"]
        Viz["2D / 3D 可视化"]
        Governance["治理与审计"]
    end

    Edge --> Ingest
    Manual --> Ingest
    Telemetry --> Ingest
    External --> Sync --> Ingest
    Ingest --> ObjectStore
    Ingest --> Normalize --> Store
    Store --> Derive
    Store --> Query
    Derive --> Query
    Query --> UI
    Query --> Analytics
    Query --> Viz
    Query --> Governance
```

核心含义：

- 外部输入只是事实来源，不直接等于中心对象。
- `Normalization Engine` 是中心语义层。
- PostgreSQL 是 source of truth。
- 派生视图只服务查询和展示，不反向成为事实源。

---

## 3. 系统边界图

```mermaid
flowchart TB
    subgraph Outside["dayu-topology 外部"]
        Agent["边缘 Agent / Discovery"]
        Control["控制中心<br/>审批 / 编译 / 下发 / 执行跟踪"]
        Cmdb["CMDB / LDAP / IAM / Oncall"]
        Obs["Telemetry / Log / Trace / Flow"]
        Visual["可视化系统"]
    end

    subgraph Dayu["dayu-topology"]
        Model["统一对象模型"]
        Relation["统一关系图谱"]
        Resolve["Identity Resolution"]
        Query["Query / Graph / Explain API"]
        Sync["External Sync"]
    end

    Agent -- "发现结果 / 运行事实 / 软件线索" --> Model
    Obs -- "摘要观测 / 依赖证据" --> Resolve
    Cmdb -- "组织 / 责任 / 外部身份" --> Sync
    Sync --> Resolve
    Resolve --> Model
    Model --> Relation
    Relation --> Query
    Query -- "拓扑 / 责任 / 风险 / explain" --> Control
    Query -- "稳定 read model" --> Visual

    Control -. "不属于 dayu-topology" .- Control
    Agent -. "不由中心侧执行 discovery" .- Agent
```

边界结论：

- `dayu-topology` 不做边缘采集，不做控制面执行。
- `dayu-topology` 提供中心侧目录、关系、查询、同步与治理底座。
- 可视化系统消费 read model，不重新定义领域模型。

---

## 4. 逻辑模块图

```mermaid
flowchart LR
    Ingest["Ingest Gateway"]
    Normalize["Normalization Engine"]
    Store["Catalog Store"]
    Derive["Derived View Builder"]
    Query["Query API"]
    Sync["External Sync Workers"]

    Ingest --> Normalize
    Normalize --> Store
    Store --> Derive
    Store --> Query
    Derive --> Query
    Sync --> Ingest

    subgraph IngestResp["Ingest Gateway 职责"]
        IG1["协议接入"]
        IG2["鉴权"]
        IG3["幂等检查"]
        IG4["IngestEnvelope"]
    end

    subgraph NormResp["Normalization Engine 职责"]
        NE1["candidate extraction"]
        NE2["identity resolution"]
        NE3["relation materialization"]
        NE4["evidence / explain"]
    end

    subgraph StoreResp["Catalog Store 职责"]
        CS1["主对象"]
        CS2["关系边"]
        CS3["运行态"]
        CS4["同步游标"]
    end

    Ingest -.-> IngestResp
    Normalize -.-> NormResp
    Store -.-> StoreResp
```

模块原则：

- 写路径负责事实归一和主对象落库。
- 读路径负责把中心对象投影成稳定视图。
- sync 是独立运行时能力，不应绕过 normalization。

---

## 5. 主写路径 Pipeline

```mermaid
flowchart LR
    Raw["Raw Payload<br/>外部事实"]
    Envelope["IngestEnvelope<br/>source / tenant / observed_at / payload_ref"]
    Typed["Typed Raw Event<br/>schema 校验"]
    Candidate["Candidate / Evidence / Observation<br/>候选层"]
    Resolve["Identity Resolution<br/>认主机 / 服务 / 主体 / 软件"]
    Materialize["Materializer<br/>对象与关系落库"]
    SOT[("Source of Truth<br/>PostgreSQL")]
    Dead["Dead Letter / Unresolved<br/>不写正式关系"]

    Raw --> Envelope
    Envelope --> Typed
    Typed --> Candidate
    Candidate --> Resolve
    Resolve -- "matched / created" --> Materialize --> SOT
    Resolve -- "unresolved / conflicting" --> Dead
```

关键约束：

- `Candidate` 不是中心主对象。
- unresolved candidate 不允许硬写成正式关系。
- resolver 必须保留来源、置信度和 explain 信息。

---

## 6. Identity Resolution 流程

```mermaid
sequenceDiagram
    participant P as Parser / Validator
    participant C as Candidate Extractor
    participant R as Identity Resolver
    participant L as ExternalIdentityLink
    participant S as Catalog Store
    participant Q as Conflict Queue

    P->>C: typed raw event
    C->>R: candidate + identifiers
    R->>L: query external mapping
    R->>S: match by strong / composite / weak identifiers

    alt high confidence match
        R->>S: reuse internal id
        R->>L: upsert mapping
    else no match
        R->>S: assign new internal id
        R->>L: create mapping
    else conflicting candidates
        R->>Q: record conflict and evidence
        R-->>C: unresolved / conflicting
    end
```

规则层次：

- 强标识：`machine_id`、`pod_uid`、`external_id`、`purl`。
- 组合标识：`cluster + namespace + workload_kind + workload_name`。
- 弱标识：display name、binary name、email 前缀，只能辅助判断。

---

## 7. 统一模型分层图

```mermaid
flowchart TB
    subgraph Business["业务架构层"]
        BD["BusinessDomain"]
        SYS["SystemBoundary"]
        SUB["Subsystem"]
        SVC["ServiceEntity"]
    end

    subgraph Catalog["资源目录层"]
        HOST["HostInventory"]
        POD["PodInventory"]
        CLUSTER["ClusterInventory"]
        NS["NamespaceInventory"]
        WORKLOAD["WorkloadEntity"]
        SW["SoftwareEntity"]
    end

    subgraph Runtime["运行实例层"]
        HRS["HostRuntimeState"]
        PROC["ProcessRuntimeState"]
        INST["ServiceInstance"]
        CEP["ContainerRuntime"]
        EP["SvcEp / InstEp"]
    end

    subgraph Relation["关系图谱层"]
        PLACE["PodPlacement"]
        BIND["RuntimeBinding"]
        DEP["DepEdge / DepObs"]
        NET["HostNetAssoc / PodNetAssoc"]
        EVID["SoftwareEvidence"]
    end

    subgraph Governance["责任治理层"]
        SUBJECT["Subject"]
        ASSIGN["ResponsibilityAssignment"]
        LINK["ExternalIdentityLink"]
        CURSOR["ExternalSyncCursor"]
        FINDING["SoftwareVulnerabilityFinding"]
    end

    BD --> SYS --> SUB --> SVC
    SVC --> WORKLOAD
    CLUSTER --> NS --> WORKLOAD --> POD
    POD --> PLACE --> HOST
    HOST --> HRS
    SVC --> INST --> BIND
    BIND --> POD
    BIND --> PROC
    CEP --> POD
    PROC --> EVID --> SW --> FINDING
    SVC --> DEP
    HOST --> NET
    POD --> NET
    ASSIGN --> SUBJECT
    HOST --> ASSIGN
    SVC --> ASSIGN
    LINK --> SUBJECT
    CURSOR --> LINK
```

分层理解：

- 上层回答业务和系统如何组织。
- 中层回答资源、工作负载、软件是谁。
- 下层回答运行实例、依赖、责任和风险如何形成。

---

## 8. 存储分层图

```mermaid
flowchart TB
    subgraph WritePath["写路径"]
        Normalize["Normalization Engine"]
        Materializer["Materializer"]
    end

    subgraph Storage["存储层"]
        PG[("PostgreSQL<br/>source of truth")]
        OBJ[("Object Storage<br/>raw payload / snapshot")]
        Cache[("Cache<br/>optional acceleration")]
    end

    subgraph PgLogical["PostgreSQL 逻辑分区"]
        Catalog["catalog<br/>业务 / 服务 / 主机 / Pod / 软件"]
        Runtime["runtime<br/>运行态 / binding / observation"]
        Governance["governance<br/>责任 / evidence / audit 摘要"]
        Sync["sync<br/>external link / cursor / job"]
    end

    Normalize --> Materializer
    Materializer --> PG
    Materializer --> OBJ
    PG --> Catalog
    PG --> Runtime
    PG --> Governance
    PG --> Sync
    PG --> Cache
```

存储原则：

- 主对象和关系对象进入 PostgreSQL。
- 原始 payload 和大快照进入对象存储。
- 缓存只做加速，不做 source of truth。

---

## 9. 读路径与 Read Model

```mermaid
flowchart LR
    SOT[("Source of Truth<br/>PostgreSQL")]
    Builder["Derived View Builder"]
    ObjectQuery["Object Query<br/>单对象 / 列表 / 轻过滤"]
    ReadModel["Derived Read Model<br/>overview / summary / effective responsibility"]
    Explain["Explain / Graph Query<br/>evidence chain / graph traversal"]
    API["Query API"]
    Consumer["UI / 控制中心 / 分析系统 / 可视化"]

    SOT --> ObjectQuery
    SOT --> Builder --> ReadModel
    SOT --> Explain
    ReadModel --> Explain
    ObjectQuery --> API
    ReadModel --> API
    Explain --> API
    API --> Consumer
```

查询原则：

- 简单对象查询可直读主库。
- 复杂聚合与全景视图走 read model。
- explain 与 graph 查询独立分层，避免污染普通列表接口。
- Query API 不直接暴露底表。

---

## 10. External Sync 流程

```mermaid
flowchart LR
    Source["External System<br/>CMDB / LDAP / IAM / Oncall / Vuln"]
    Connector["Connector<br/>auth / paging / rate limit"]
    Stage["Fetch & Stage<br/>raw payload / metadata"]
    Normalize["Normalize & Resolve<br/>external id -> internal id"]
    Persist["Persist<br/>upsert source of truth"]
    Cursor["Advance Cursor<br/>only after persist success"]
    Store[("PostgreSQL")]
    Replay["Replay<br/>from staged payload"]
    Fail["Failure<br/>isolate source / keep cursor"]

    Source --> Connector --> Stage --> Normalize --> Persist --> Store --> Cursor
    Stage --> Replay --> Normalize
    Connector -. failure .-> Fail
    Normalize -. conflict .-> Fail
    Persist -. failure .-> Fail
    Fail -. no cursor advance .-> Cursor
```

同步原则：

- connector 只负责拉取，不负责绕过中心模型写库。
- staged payload 支持失败重放。
- cursor 只在主写成功后推进。
- 一个源失败不阻塞其他源。

---

## 11. 第一版部署演进图

```mermaid
flowchart TB
    subgraph Phase1["Phase 1：单体起步"]
        Mono["dayu-topology-server<br/>API + ingest + normalize + query"]
        PG1[("PostgreSQL")]
        OBJ1[("Object Storage")]
        Mono --> PG1
        Mono --> OBJ1
    end

    subgraph Phase2["Phase 2：单体 + Worker"]
        API2["dayu-topology-server"]
        Worker2["dayu-topology-worker<br/>batch normalize / derive / explain rebuild"]
        PG2[("PostgreSQL")]
        OBJ2[("Object Storage")]
        API2 --> PG2
        Worker2 --> PG2
        API2 --> OBJ2
        Worker2 --> OBJ2
    end

    subgraph Phase3["Phase 3：API / Worker / Sync 三分"]
        API3["dayu-topology-api"]
        Worker3["dayu-topology-worker"]
        Sync3["dayu-topology-sync"]
        PG3[("PostgreSQL")]
        OBJ3[("Object Storage")]
        API3 --> PG3
        Worker3 --> PG3
        Sync3 --> PG3
        Sync3 --> OBJ3
        Worker3 --> OBJ3
    end

    Phase1 --> Phase2 --> Phase3
```

演进原则：

- 第一版单体优先。
- 逻辑边界先清楚，物理拆分按压力与隔离需求推进。
- sync、worker、query 都应能独立扩展，但不应提前复杂化。

---

## 12. 代码与 crate 映射图

```mermaid
flowchart LR
    Domain["topology-domain<br/>领域对象 / contract / DTO"]
    Storage["topology-storage<br/>repository / migration / Postgres"]
    API["topology-api<br/>ingest / query / view / explain"]
    Sync["topology-sync<br/>connector / runner / cursor"]
    App["topology-app<br/>config / role / process entry"]

    Domain --> Storage
    Domain --> API
    Domain --> Sync
    Storage --> API
    Storage --> Sync
    API --> App
    Sync --> App
    Storage --> App
```

crate 原则：

- `topology-domain` 是领域语义单一来源。
- `topology-storage` 只做存储 contract 和实现。
- `topology-api` 提供 ingest 与 query 能力。
- `topology-sync` 提供外部同步能力。
- `topology-app` 只做装配与运行角色选择。

---

## 13. 第一版开发路线图

```mermaid
gantt
    title dayu-topology 第一版开发路线
    dateFormat  YYYY-MM-DD
    axisFormat  W%W

    section Foundation
    Phase 0 固定实现基线           :p0, 2026-04-20, 1w
    Phase 1 领域模型与主存储       :p1, after p0, 1w

    section Core Loop
    Phase 2 最小写路径闭环         :p2, after p1, 2w
    Phase 3 最小 Query API         :p3, after p2, 1w

    section Integration
    Phase 4 外部同步基础能力       :p4, after p3, 2w
    Phase 5 派生视图与治理扩展     :p5, after p4, 2w
```

路线原则：

- 先让模型和主库站稳。
- 再打通 ingest 到 query 的最小闭环。
- sync 和 derived view 建立在稳定 source of truth 之上。

---

## 14. 传播建议

对外介绍时建议按以下顺序使用图：

1. 一页总览
2. 系统边界图
3. 主写路径 Pipeline
4. 统一模型分层图
5. 读路径与 Read Model
6. External Sync 流程
7. 第一版部署演进图

对研发评审时建议补充：

- Identity Resolution 流程
- 存储分层图
- 代码与 crate 映射图
- 第一版开发路线图
