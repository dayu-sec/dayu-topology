# dayu-topology 待办清单

## 1. 文档目的

本文档把 [`execution-plan.md`](./execution-plan.md) 中的工作包进一步压缩成可跟踪的 backlog。

目标是明确：

- 当前应优先做哪些任务
- 每个任务的优先级、依赖和验收条件
- 任务如何分配到各 crate

相关文档：

- [`development-plan.md`](./development-plan.md)
- [`execution-plan.md`](./execution-plan.md)

---

## 2. 使用方式

建议每个任务至少维护以下字段：

- `Priority`
- `Status`
- `Owner`
- `Depends On`
- `Acceptance`

状态建议统一为：

- `todo`
- `in_progress`
- `blocked`
- `done`

负责人建议先填：

- `TBD`

---

## 3. P0：最小闭环必须完成

这些任务直接决定第一版是否能形成最小可运行闭环。

| ID | Priority | Task | Crate | Status | Owner | Depends On | Acceptance |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `P0-01` | `P0` | 定义第一版核心领域对象与枚举 | `topology-domain` | `todo` | `TBD` | - | `business/service/host/subject/runtime_binding/external_sync` 等核心对象均有稳定类型定义 |
| `P0-02` | `P0` | 定义共享值对象与时间语义类型 | `topology-domain` | `todo` | `TBD` | `P0-01` | `tenant/environment/validity/observed_at/confidence` 不再散落为裸 `String` |
| `P0-03` | `P0` | 定义 ingest/candidate/resolution/read-model contract | `topology-domain` | `todo` | `TBD` | `P0-01` | 输入、中间层和查询 DTO 有单独类型，API 和 storage 不自定义领域语义 |
| `P0-04` | `P0` | 建立 migration 目录与初始化方式 | `topology-storage` | `done` | `TBD` | `P0-01` | 空库可初始化，migration 方案固定 |
| `P0-05` | `P0` | 落第一批 catalog 主表 | `topology-storage` | `done` | `TBD` | `P0-04` | `business/system/service/cluster/namespace/workload/pod/host/subject` 表落地 |
| `P0-06` | `P0` | 落第一批 runtime/governance/sync 表 | `topology-storage` | `done` | `TBD` | `P0-04` | `host_runtime_state/runtime_binding/responsibility_assignment/external_identity_link/external_sync_cursor` 表落地 |
| `P0-07` | `P0` | 固定关键唯一键与时间约束 | `topology-storage` | `done` | `TBD` | `P0-05,P0-06` | 数据库层保障幂等与有效期基础约束 |
| `P0-08` | `P0` | 实现基础 repository trait | `topology-storage` | `done` | `TBD` | `P0-05,P0-06` | `CatalogStore/RuntimeStore/GovernanceStore/SyncStore` 可编译可调用 |
| `P0-09` | `P0` | 实现 Postgres upsert 与基础查询 | `topology-storage` | `in_progress` | `TBD` | `P0-07,P0-08` | upsert/query SQL contract 已落地；具体 PostgreSQL client 执行器待选型接入 |
| `P0-10` | `P0` | 实现 ingest submit 入口 | `topology-api` | `done` | `TBD` | `P0-03,P0-09` | 可接收 `IngestEnvelope` 并记录 ingest job |
| `P0-11` | `P0` | 实现业务目录 candidate extractor | `topology-api` | `done` | `TBD` | `P0-10` | 可从目录导入 payload 提取 business/system/service candidate |
| `P0-12` | `P0` | 实现主机目录 candidate extractor | `topology-api` | `done` | `TBD` | `P0-10` | 可从主机导入 payload 提取 host candidate |
| `P0-13` | `P0` | 实现 host/service/subject 最小 resolver | `topology-api` | `todo` | `TBD` | `P0-03,P0-09,P0-11,P0-12` | 至少支持强标识和组合标识规则 |
| `P0-14` | `P0` | 实现 materializer | `topology-api` | `todo` | `TBD` | `P0-13` | 已解析对象可写入主库，未解析对象不写正式关系 |
| `P0-15` | `P0` | 打通业务目录导入链路 | `topology-api` | `todo` | `TBD` | `P0-11,P0-13,P0-14` | 导入业务目录后可查到 business/service 数据 |
| `P0-16` | `P0` | 打通主机目录导入链路 | `topology-api` | `todo` | `TBD` | `P0-12,P0-13,P0-14` | 导入主机目录后可查到 host 数据 |
| `P0-17` | `P0` | 实现核心对象查询 API | `topology-api` | `todo` | `TBD` | `P0-09` | 支持查询 `host/service/business/subject` |
| `P0-18` | `P0` | 实现基础 topology/governance view | `topology-api` | `todo` | `TBD` | `P0-17,P0-14` | 支持 `host_topology_view/service_topology_view/effective_responsibility_view` |
| `P0-19` | `P0` | 实现单体模式启动入口 | `topology-app` | `todo` | `TBD` | `P0-09,P0-17` | 本地可启动 API + storage 的最小单体 |
| `P0-20` | `P0` | 建立最小 e2e smoke test | `topology-app` | `todo` | `TBD` | `P0-15,P0-16,P0-18,P0-19` | 本地或 CI 中可验证导入与查询最小闭环 |

---

## 4. P1：第一版增强能力

这些任务不决定“能不能跑起来”，但决定第一版是否具备可联调和可运维能力。

| ID | Priority | Task | Crate | Status | Owner | Depends On | Acceptance |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `P1-01` | `P1` | 实现责任关系导入 candidate 与 materialize | `topology-api` | `todo` | `TBD` | `P0-14` | 可导入 `ResponsibilityAssignment` |
| `P1-02` | `P1` | 实现 `HostRuntimeState` 写入与查询 | `topology-storage`,`topology-api` | `todo` | `TBD` | `P0-06,P0-09` | 运行态快照可写入并按时间查询 |
| `P1-03` | `P1` | 实现 `ServiceInstance` 与 `RuntimeBinding` 存储与查询 | `topology-storage`,`topology-api` | `todo` | `TBD` | `P0-06,P0-09` | 可查询实例和绑定关系 |
| `P1-04` | `P1` | 实现 runtime binding explain 最小版 | `topology-api` | `todo` | `TBD` | `P1-03` | 返回绑定来源、规则和关键证据摘要 |
| `P1-05` | `P1` | 实现 resolution explain 最小版 | `topology-api` | `todo` | `TBD` | `P0-13` | 可解释为何命中某内部对象 |
| `P1-06` | `P1` | 实现 connector trait 与 sync runner | `topology-sync` | `todo` | `TBD` | `P0-03,P0-09` | sync job 可独立执行 fetch/resolve/persist 流程 |
| `P1-07` | `P1` | 实现 staged payload contract | `topology-sync` | `todo` | `TBD` | `P1-06` | 外部 payload 可被记录、重放和追踪 |
| `P1-08` | `P1` | 实现 CMDB connector 最小版 | `topology-sync` | `todo` | `TBD` | `P1-06,P1-07` | 可同步 host 与默认责任归属 |
| `P1-09` | `P1` | 实现 LDAP/IAM connector 最小版 | `topology-sync` | `todo` | `TBD` | `P1-06,P1-07` | 可同步 user/team 到 `Subject` |
| `P1-10` | `P1` | 实现 cursor 推进、失败隔离与重试 | `topology-sync` | `todo` | `TBD` | `P1-08,P1-09` | 同步失败不会推进 cursor，重试保持幂等 |
| `P1-11` | `P1` | 实现配置结构与角色模式启动 | `topology-app` | `todo` | `TBD` | `P0-19` | 支持单体、API only、sync only 角色 |
| `P1-12` | `P1` | 增加 request/pipeline/sync 结构化日志 | `topology-app` | `todo` | `TBD` | `P0-19,P1-06` | 关键路径具备统一日志字段 |
| `P1-13` | `P1` | 增加基础 metrics | `topology-app` | `todo` | `TBD` | `P0-19` | 至少暴露 request latency、ingest 成功率、sync lag |
| `P1-14` | `P1` | 增加 audit event 基础能力 | `topology-api`,`topology-app` | `todo` | `TBD` | `P1-01,P1-04,P1-05` | 手工导入、敏感 explain、责任修正可审计 |
| `P1-15` | `P1` | 补本地启动与样例导入文档 | `doc` | `todo` | `TBD` | `P0-19,P0-20` | 新开发者可按文档跑起最小闭环 |

---

## 5. P2：扩展与优化任务

这些任务适合在第一版最小闭环稳定后推进。

| ID | Priority | Task | Crate | Status | Owner | Depends On | Acceptance |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `P2-01` | `P2` | 增加 `business_overview_view` | `topology-api` | `todo` | `TBD` | `P0-18` | 可聚合业务、系统、服务与摘要信息 |
| `P2-02` | `P2` | 增加 dependency observation 模型 | `topology-domain`,`topology-storage` | `todo` | `TBD` | `P1-03` | `DepObs/DepEdge` 有明确存储与查询语义 |
| `P2-03` | `P2` | 增加 dependency explain 查询 | `topology-api` | `todo` | `TBD` | `P2-02` | 可解释一条依赖为何成立 |
| `P2-04` | `P2` | 增加 software normalization 基础模型 | `topology-domain`,`topology-storage` | `todo` | `TBD` | `P0-09` | `software_product/version/artifact` 落地 |
| `P2-05` | `P2` | 增加 vulnerability finding 基础链路 | `topology-sync`,`topology-storage`,`topology-api` | `todo` | `TBD` | `P2-04,P1-06` | 漏洞情报可同步并形成 finding |
| `P2-06` | `P2` | 增加 `software_risk_view` | `topology-api` | `todo` | `TBD` | `P2-05` | 可查询软件风险摘要 |
| `P2-07` | `P2` | 增加 read-model 物化或缓存策略 | `topology-storage`,`topology-api` | `todo` | `TBD` | `P2-01,P2-06` | 热点视图有明确加速方案 |
| `P2-08` | `P2` | 增加 object storage 原始 payload 归档 | `topology-sync`,`topology-app` | `todo` | `TBD` | `P1-07` | 原始 payload 有外部引用并支持回放 |
| `P2-09` | `P2` | 增加权限边界与敏感查询控制 | `topology-api`,`topology-app` | `todo` | `TBD` | `P1-14` | explain 与敏感对象查询具备单独授权控制 |

---

## 6. 推荐分工

如果是 2-4 人小团队，建议按下面方式分工：

### 角色 A：领域与存储

负责：

- `P0-01` 到 `P0-09`
- `P1-02`
- `P1-03`
- `P2-04`

### 角色 B：API 与写路径

负责：

- `P0-10` 到 `P0-18`
- `P1-01`
- `P1-04`
- `P1-05`

### 角色 C：应用装配与测试

负责：

- `P0-19`
- `P0-20`
- `P1-11`
- `P1-12`
- `P1-13`
- `P1-15`

### 角色 D：同步能力

负责：

- `P1-06` 到 `P1-10`
- `P2-05`
- `P2-08`

---

## 7. 建议看板顺序

建议按以下顺序拉任务进入迭代：

### Sprint 1

- `P0-01`
- `P0-02`
- `P0-03`
- `P0-04`
- `P0-05`
- `P0-06`

### Sprint 2

- `P0-07`
- `P0-08`
- `P0-09`
- `P0-10`
- `P0-11`
- `P0-12`

### Sprint 3

- `P0-13`
- `P0-14`
- `P0-15`
- `P0-16`

### Sprint 4

- `P0-17`
- `P0-18`
- `P0-19`
- `P0-20`

### Sprint 5+

- 按 `P1` 顺序推进
- `P2` 只在 `P0` 和关键 `P1` 稳定后再进入

---

## 8. 当前建议

当前建议固定为：

- backlog 管理先以 `P0` 为主，不要同时拉入过多 `P1/P2`
- 每个任务必须有明确验收条件，避免“代码写了但能力没闭环”
- 第一版只要 `P0` 全部完成，就已经具备内部演示和继续扩展的基础
