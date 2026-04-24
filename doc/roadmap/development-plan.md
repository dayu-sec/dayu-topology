# dayu-topology 开发计划

## 1. 文档目的

本文档基于当前 `dayu-topology/doc` 下的架构与模型文档，整理第一版可执行开发计划。

目标是明确：

- 第一版优先实现哪些对象与能力
- 为什么开发顺序应如此安排
- 每个阶段要产出什么
- 当前仓库结构下各模块应如何落地

相关文档：

- [`../README.md`](../README.md)
- [`../architecture/project-charter.md`](../architecture/project-charter.md)
- [`../architecture/system-architecture.md`](../architecture/system-architecture.md)
- [`../architecture/dataflow-and-pipeline-architecture.md`](../architecture/dataflow-and-pipeline-architecture.md)
- [`../architecture/unified-model-overview.md`](../architecture/unified-model-overview.md)
- [`../model/unified-topology-schema.md`](../model/unified-topology-schema.md)
- [`bootstrap-plan.md`](./bootstrap-plan.md)

---

## 2. 当前判断

当前仓库处于“文档先行、代码骨架已建立、实现尚未展开”的阶段。

现状特点：

- 架构边界已经比较清楚
- 统一模型方向已经明确
- crate 拆分已经预留
- 代码中只有少量示例 struct 和 trait

因此第一版开发重点不应放在：

- 过早拆分服务
- 过早接入过多外部源
- 过早建设复杂图查询能力

而应优先放在：

- 固定统一模型与主键语义
- 打通最小写路径闭环
- 提供最小稳定查询面
- 为后续 sync、derived view、explain 预留边界

---

## 3. 第一版范围建议

### 3.1 第一版优先对象

第一版建议优先实现以下对象域：

- `business / system / subsystem / service`
- `cluster / namespace / workload / pod`
- `host / host_runtime`
- `subject / responsibility_assignment`
- `external_identity_link / external_sync_cursor`
- `service_instance / runtime_binding`

### 3.2 第一版优先能力

第一版建议优先实现以下能力：

- PostgreSQL 主库存储
- 最小 ingest envelope 与 normalize 闭环
- 最小 identity resolution
- 最小 Query API
- 最小 external sync 基础设施

### 3.3 第一版延后能力

以下能力建议放在第一版后半段或下一阶段：

- 复杂依赖图 explain
- 全量漏洞情报接入
- 风险传播视图
- 通用图查询语言
- 大规模缓存与多级存储优化

---

## 4. 开发总原则

第一版建议固定以下开发原则：

- 先统一模型，再扩展数据源
- 先做 source of truth，再做 derived view
- 先做单体闭环，再决定是否拆服务
- 先保证幂等、一致性与 explain，再追求吞吐
- unresolved candidate 不进入正式关系

一句话说：

- 第一版首先要做“可信、可解释、可收敛”的中心目录，不是做一个大而全的平台。

---

## 5. 分阶段开发计划

## 5.1 Phase 0：固定实现基线

目标：

- 把文档约束收敛成实现 contract

主要工作：

- 固定第一版对象范围与非目标
- 固定主键、唯一键、有效期与快照时间语义
- 固定 `tenant` 与 `environment` 边界
- 固定 ingest / sync / query / derive 的职责边界
- 补充简短 ADR 或 design note

建议先固定的设计决议：

- 内部对象统一使用 `uuid`
- 关系对象统一使用 `valid_from / valid_to`
- 运行态统一使用 `observed_at`
- sync cursor 只在持久化成功后推进
- unresolved candidate 只保留在 candidate/evidence 层

完成标志：

- 团队对“第一版做什么、不做什么”达成一致
- 数据语义和边界不再频繁变动

---

## 5.2 Phase 1：打通领域模型与主存储

目标：

- 建立第一版 source of truth 基座

主要工作：

- 扩充 `topology-domain`
- 落 PostgreSQL schema 与 migration
- 实现 `topology-storage` 的 repository 接口和 Postgres 版本
- 为主对象、关系对象和运行态对象建立存储测试

建议优先落地的领域对象：

- `BusinessDomain`
- `SystemBoundary`
- `Subsystem`
- `ServiceEntity`
- `ClusterInventory`
- `NamespaceInventory`
- `WorkloadEntity`
- `PodInventory`
- `HostInventory`
- `HostRuntimeState`
- `Subject`
- `ResponsibilityAssignment`
- `ExternalIdentityLink`
- `ExternalSyncCursor`
- `ServiceInstance`
- `RuntimeBinding`

建议优先固定的数据库能力：

- 主对象唯一约束
- 幂等 upsert
- 关系对象时间段关闭与续期
- 运行态按 `observed_at` 写入
- 基础分页与过滤查询

完成标志：

- 核心对象可稳定入库
- 约束、索引和时间语义基本可用

---

## 5.3 Phase 2：实现最小写路径闭环

目标：

- 从输入事实到主库对象形成最小闭环

主要工作：

- 实现 `IngestEnvelope`
- 实现 parser / validator
- 实现 candidate extractor
- 实现最小 identity resolver
- 实现 materializer

第一批建议支持的输入链路：

- 手工或批量导入业务目录
- 手工或批量导入主机目录
- 手工或批量导入责任关系

第一批建议覆盖的 identity resolution：

- `host`
- `service`
- `subject`
- `workload`

规则建议按三层实现：

- 强标识规则
- 组合标识规则
- 弱标识辅助规则

关键要求：

- resolution 失败可降级，但不能静默误归属
- materializer 不得把 unresolved candidate 写成正式关系
- 所有高语义关系保留来源和置信度

完成标志：

- 至少两条输入链路能从导入一路走到主库
- 核心 resolution 结果可 explain

---

## 5.4 Phase 3：实现最小 Query API

目标：

- 对外提供稳定读接口，而不是直接暴露底表

主要工作：

- 实现 catalog query
- 实现基础 topology view
- 实现 governance query
- 实现轻量 explain 返回

建议第一批对象查询：

- `host`
- `service`
- `business`
- `subject`

建议第一批视图：

- `host_topology_view`
- `service_topology_view`
- `effective_responsibility_view`

建议第一批 API 分组：

- `Catalog API`
- `Topology API`
- `Governance API`
- `Explain API`

建议第一版限制：

- 不直接开放底层 schema
- 不提供通用图查询语言
- explain 查询限深、分页

完成标志：

- 上层系统可以直接消费稳定读模型
- 不需要自行拼接底层表关系

---

## 5.5 Phase 4：实现外部同步基础能力

目标：

- 把外部系统事实安全同步到中心模型

主要工作：

- 实现 `topology-sync` 基础执行框架
- 拆分 connector / fetch-stage / normalize-resolve / persist-cursor
- 落地 staged payload 存储与重放机制
- 实现源级隔离和失败恢复

第一批建议接入的同步源：

- `CMDB`
- `LDAP / IAM`

第一批建议同步的对象域：

- 主机基础归属
- 用户与团队主体
- 默认责任关系

必须固定的同步原则：

- 每个源独立 cursor
- cursor 只在持久化成功后推进
- staged payload 可重放
- 一个源失败不污染已有主对象

来源优先级建议固定为：

1. `manual`
2. `cmdb_sync`
3. `oncall_sync`
4. `rule_derived`

完成标志：

- 外部系统数据能幂等进入中心模型
- 同步失败时可隔离、可恢复、可重放

---

## 5.6 Phase 5：派生视图与治理扩展

目标：

- 增加面向控制中心、分析系统和治理系统的复合读能力

主要工作：

- 构建 `business_overview_view`
- 构建 `software_risk_view`
- 构建 `dependency_explain_view`
- 接入 dependency observation
- 接入 software normalization 与 vulnerability finding
- 增加 metrics、structured logs 与 audit events

建议执行顺序：

- 先派生责任与业务摘要视图
- 再做依赖 explain
- 再做漏洞与风险聚合

完成标志：

- 具备跨对象域联合查询能力
- 可支撑风险、责任、依赖的综合分析

---

## 6. 按 crate 的落地建议

### 6.1 `topology-domain`

负责：

- 领域对象定义
- 枚举和值对象
- ingest / candidate / resolver / read model contract

第一批应补齐：

- 主对象 struct
- 关系对象 struct
- 运行态对象 struct
- 查询 DTO
- resolver 输入输出类型

### 6.2 `topology-storage`

负责：

- repository trait
- Postgres 存储实现
- migration 与 schema 管理
- 幂等 upsert 与读查询

第一批应补齐：

- catalog repository
- runtime repository
- governance repository
- sync repository

### 6.3 `topology-api`

负责：

- Query API
- ingest 接入层
- API DTO 与 handler
- 鉴权与审计接入点

第一批应补齐：

- catalog query handler
- topology view handler
- governance query handler
- ingest submit handler

### 6.4 `topology-sync`

负责：

- connector 调度
- fetch/stage
- normalize / persist
- cursor 推进

第一批应补齐：

- connector trait
- staged payload contract
- sync job runner
- cursor repository

### 6.5 `topology-app`

负责：

- 进程启动入口
- 配置装配
- server / worker / sync 角色编排

第一版建议：

- 先支持单体模式
- 代码结构上预留 API / Worker / Sync 三种运行角色

---

## 7. 建议排期

如果按小团队、单体优先方式推进，建议排期如下：

### 第 1-2 周

- 完成 Phase 0
- 完成 Phase 1

### 第 3-4 周

- 完成 Phase 2

### 第 5 周

- 完成 Phase 3

### 第 6-7 周

- 完成 Phase 4

### 第 8 周以后

- 按优先级推进 Phase 5

说明：

- 该排期适合“先建立最小闭环”的推进方式
- 若外部源接入复杂度高，Phase 4 需要单独留缓冲

---

## 8. 当前最优先的三件事

结合当前仓库现状，最应该先做的是：

1. 把第一版领域模型和 PostgreSQL migration 落下来
2. 打通业务目录导入与主机目录导入两条最小 ingest 闭环
3. 提供 `host / service / business / responsibility` 的最小查询 API

原因：

- 这三件事完成后，系统才真正拥有可验证的中心目录能力
- 后续 sync、derive、explain 都能建立在稳定基座之上

---

## 9. 当前建议

当前建议固定为：

- 第一版以单体优先，不预设拆服务
- PostgreSQL 作为唯一 source of truth 主库
- 开发顺序按“模型与存储 -> 写路径 -> 查询 -> 同步 -> 派生扩展”推进
- 任何新增能力都不应绕过统一模型、identity resolution 与 materialization 主路径
