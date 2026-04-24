# dayu-topology 执行计划

## 1. 文档目的

本文档在 [`development-plan.md`](./development-plan.md) 的基础上，进一步把第一版开发工作细化成可执行任务清单。

目标是明确：

- 每个阶段具体要做哪些任务
- 各任务之间的前置依赖是什么
- 各 crate 应承担什么实现责任
- 每个阶段如何验收
- 小团队如何按周推进

相关文档：

- [`development-plan.md`](./development-plan.md)
- [`bootstrap-plan.md`](./bootstrap-plan.md)
- [`../architecture/system-architecture.md`](../architecture/system-architecture.md)
- [`../architecture/dataflow-and-pipeline-architecture.md`](../architecture/dataflow-and-pipeline-architecture.md)
- [`../model/unified-topology-schema.md`](../model/unified-topology-schema.md)

---

## 2. 执行策略

第一版建议采用以下执行策略：

- 以单体闭环为目标组织开发
- 以 PostgreSQL 主库和最小 API 为第一里程碑
- 每阶段只引入少量新对象域，避免同时铺开
- 每个阶段都要求“可运行、可测试、可解释”，而不只是“代码已写”

整体推进顺序固定为：

1. 领域模型与存储
2. 写路径闭环
3. 查询闭环
4. 外部同步
5. 派生视图与治理扩展

---

## 3. 总任务分解

第一版任务建议拆成八个工作包：

- `WP-01` 领域模型收敛
- `WP-02` 数据库存储基座
- `WP-03` Ingest 与 Normalize 主写路径
- `WP-04` Query API 与读模型
- `WP-05` External Sync 基础设施
- `WP-06` 应用装配与运行模式
- `WP-07` 可观测性、审计与测试
- `WP-08` 文档、样例与演示数据

各工作包之间的依赖关系：

```text
WP-01 -> WP-02 -> WP-03 -> WP-04
                  \
                   -> WP-05

WP-02 -> WP-06
WP-03 -> WP-07
WP-04 -> WP-07
WP-05 -> WP-07

WP-01/WP-03/WP-04 -> WP-08
```

---

## 4. 工作包明细

## 4.1 WP-01：领域模型收敛

目标：

- 把文档中的第一版对象模型收敛成代码级 contract

主要产出：

- `topology-domain` 中的稳定领域对象
- 枚举和值对象定义
- ingest / candidate / resolver / read-model 类型定义

任务清单：

1. 建立基础共享类型
- `TenantId`
- `EnvironmentId`
- `ObservedAt`
- `ValidityWindow`
- `Confidence`
- `SourceKind`

2. 补齐目录对象
- `BusinessDomain`
- `SystemBoundary`
- `Subsystem`
- `ServiceEntity`
- `ClusterInventory`
- `NamespaceInventory`
- `WorkloadEntity`
- `PodInventory`
- `HostInventory`
- `Subject`

3. 补齐运行态对象
- `HostRuntimeState`
- `ServiceInstance`
- `ContainerRuntime`
- `ProcessRuntimeState`

4. 补齐关系对象
- `ResponsibilityAssignment`
- `RuntimeBinding`
- `WorkloadPodMembership`
- `PodPlacement`

5. 补齐同步与解释对象
- `ExternalIdentityLink`
- `ExternalSyncCursor`
- `RuntimeBindingEvidence`
- `ResolutionResult`

6. 补齐输入与中间层类型
- `IngestEnvelope`
- `BusinessCatalogCandidate`
- `HostCandidate`
- `SubjectCandidate`
- `ResponsibilityAssignmentCandidate`

7. 补齐查询 DTO
- `CatalogSummary`
- `BusinessOverviewView`
- `HostTopologyView`
- `ServiceTopologyView`
- `EffectiveResponsibilityView`

前置依赖：

- 无

验收标准：

- 第一版核心对象在代码中均有明确 struct/enum 定义
- 关键字段含义与文档一致
- 不再使用松散 `String` 表达核心枚举语义

---

## 4.2 WP-02：数据库存储基座

目标：

- 建立可承载 source of truth 的 PostgreSQL 存储基座

主要产出：

- migration
- schema
- repository trait
- Postgres repository 实现

任务清单：

1. 建立 migration 管理方式
- 选定 migration 目录结构
- 固定建表、索引、回滚策略

2. 落第一批主表
- `business_domain`
- `system_boundary`
- `subsystem`
- `service_entity`
- `cluster_inventory`
- `namespace_inventory`
- `workload_entity`
- `pod_inventory`
- `host_inventory`
- `subject`

3. 落第一批运行态与关系表
- `host_runtime_state`
- `service_instance`
- `runtime_binding`
- `responsibility_assignment`
- `workload_pod_membership`
- `pod_placement`

4. 落同步相关表
- `external_identity_link`
- `external_sync_cursor`
- `ingest_job`
- `sync_job`

5. 固定关键约束
- 主对象唯一键
- `host_runtime_state(host_id, observed_at)` 唯一键
- `runtime_binding` 的有效期约束
- `external_identity_link(system_type, object_type, external_id)` 唯一键

6. 实现 repository trait
- `CatalogStore`
- `RuntimeStore`
- `GovernanceStore`
- `SyncStore`

7. 实现 Postgres 版本
- upsert
- 按主键查询
- 简单列表
- 按时间窗口查询运行态

8. 建立存储测试
- migration smoke test
- upsert 幂等测试
- 唯一键冲突测试
- 有效期续期/关闭测试

前置依赖：

- `WP-01`

验收标准：

- 空库可以一键初始化
- 第一批核心对象可稳定 upsert
- 关键唯一约束和时间语义由数据库保障

---

## 4.3 WP-03：Ingest 与 Normalize 主写路径

目标：

- 把输入事实转换为中心主对象与关系对象

主要产出：

- intake 接口
- parser / validator
- candidate extractor
- identity resolver
- materializer

任务清单：

1. 实现 ingest submit 入口
- 接收 `IngestEnvelope`
- 记录 ingest job 元数据
- 保存原始 payload 引用或 inline payload

2. 实现 parser / validator
- schema 校验
- 时间戳规范化
- tenant/environment 校验
- dead-letter 输出

3. 实现第一批 candidate extractor
- 业务目录导入 candidate
- 主机目录导入 candidate
- 责任关系导入 candidate

4. 实现第一批 identity resolver
- host resolver
- service resolver
- subject resolver
- workload resolver

5. 实现 conflict handling
- 强标识命中
- 组合标识匹配
- 弱标识辅助评分
- unresolved / conflicting 状态输出

6. 实现 materializer
- 目录对象写入
- 关系对象写入
- 运行态对象写入
- evidence / resolution metadata 写入

7. 实现 explain 基础链路
- 记录命中规则
- 记录关键标识
- 记录冲突候选摘要

8. 建立端到端测试
- 导入业务目录成功
- 导入主机目录成功
- 导入责任关系成功
- 重复导入保持幂等
- 冲突导入进入 unresolved/冲突状态

前置依赖：

- `WP-01`
- `WP-02`

验收标准：

- 至少两条输入链路可从提交一路写入主库
- 重放同一输入不产生重复主对象
- resolution 结果有最小 explain 信息

---

## 4.4 WP-04：Query API 与读模型

目标：

- 对外提供稳定读接口和最小视图能力

主要产出：

- catalog query
- topology query
- governance query
- explain query

任务清单：

1. 固定 API 分组
- `Catalog API`
- `Topology API`
- `Governance API`
- `Explain API`

2. 实现第一批对象查询
- `GET /hosts`
- `GET /hosts/{id}`
- `GET /services`
- `GET /services/{id}`
- `GET /businesses`
- `GET /subjects`

3. 实现第一批视图查询
- `GET /topology/hosts/{id}`
- `GET /topology/services/{id}`
- `GET /governance/effective-responsibility`

4. 实现 explain 基础查询
- `GET /explain/runtime-binding/{binding_id}`
- `GET /explain/resolution/{object_kind}/{id}`

5. 实现读模型组装层
- 对象查询直读主表
- 视图查询统一走组装器
- explain 查询统一走 evidence 组装器

6. 建立 API 测试
- handler 单测
- repository 集成测试
- API contract 测试
- 简单分页过滤测试

前置依赖：

- `WP-01`
- `WP-02`
- `WP-03`

验收标准：

- 上层调用方可不依赖底表直接查询核心对象
- 视图与 explain 返回结构稳定
- 基础查询具备分页和错误响应规范

---

## 4.5 WP-05：External Sync 基础设施

目标：

- 建立最小可用的外部同步执行框架

主要产出：

- connector contract
- staged payload 机制
- cursor 推进机制
- CMDB/LDAP 第一批同步器

任务清单：

1. 定义 connector trait
- 认证
- 拉取分页
- 增量 cursor
- 全量校准

2. 定义 staged payload contract
- payload metadata
- payload ref
- fetch timestamp
- replay ability

3. 实现 sync job runner
- 调度
- fetch & stage
- normalize & resolve
- persist & cursor advance

4. 实现第一批 connector
- CMDB connector
- LDAP/IAM connector

5. 实现第一批同步映射
- `cmdb host -> HostInventory`
- `cmdb owner team -> ResponsibilityAssignment`
- `ldap user/team -> Subject`

6. 实现错误隔离与重试
- 源级失败隔离
- cursor 不前移
- staged payload 可重放

7. 建立同步测试
- 初次全量同步
- 增量同步
- 重试幂等
- cursor 失败恢复

前置依赖：

- `WP-01`
- `WP-02`
- `WP-03`

验收标准：

- 至少一个组织类源和一个资源类源可完成同步
- cursor 推进和重试语义正确
- 同步失败不会破坏已有主对象

---

## 4.6 WP-06：应用装配与运行模式

目标：

- 让系统具备可启动、可配置、可切换角色的运行入口

主要产出：

- 统一配置结构
- 单体模式启动入口
- API / worker / sync 角色装配

任务清单：

1. 设计配置结构
- server 配置
- database 配置
- object storage 配置
- auth 配置
- sync 配置

2. 实现 `topology-app` 启动入口
- 单体模式
- API only 模式
- worker only 模式
- sync only 模式

3. 实现依赖装配
- repository 注入
- handler 注入
- sync runner 注入

4. 补基础健康检查
- liveness
- readiness

5. 增加开发环境启动方式
- 本地配置模板
- 最小启动说明

前置依赖：

- `WP-02`
- `WP-03`
- `WP-04`

验收标准：

- 本地可启动单体模式
- 可通过配置切换到独立角色模式

---

## 4.7 WP-07：可观测性、审计与测试

目标：

- 让系统具备最小排障、审计和质量保障能力

主要产出：

- metrics
- structured logs
- audit events
- 自动化测试基线

任务清单：

1. 固定日志规范
- request log
- ingest pipeline log
- resolution log
- sync log

2. 固定 metrics
- request latency
- ingest success/failure
- resolution conflict count
- sync lag
- derive lag 预留指标

3. 固定 audit event
- 手工导入
- 手工责任修正
- 敏感 explain 查询
- sync replay

4. 建立测试分层
- domain 单测
- storage 集成测试
- api 测试
- e2e smoke test

5. 建立示例场景测试
- 小型业务目录
- 小型主机与责任关系
- 简单同步场景

前置依赖：

- `WP-03`
- `WP-04`
- `WP-05`

验收标准：

- 关键路径具备日志和指标
- 高风险治理动作可审计
- 至少具备一套端到端 smoke test

---

## 4.8 WP-08：文档、样例与演示数据

目标：

- 提供能支持开发、联调和演示的文档与样例

主要产出：

- API 示例
- 导入样例
- 测试 fixture
- 本地演示数据集

任务清单：

1. 补导入样例
- 业务目录样例
- 主机目录样例
- 责任关系样例

2. 补 API 示例
- catalog 查询示例
- topology 查询示例
- explain 查询示例

3. 补 fixture
- 最小租户样例
- 多业务样例
- 冲突 identity 样例

4. 补开发说明
- 本地启动
- 初始化数据库
- 导入样例数据
- 验证查询结果

前置依赖：

- `WP-01`
- `WP-03`
- `WP-04`

验收标准：

- 新加入开发者能按文档在本地跑起最小闭环
- 演示环境有稳定样例数据

---

## 5. 按 crate 拆分任务

## 5.1 `topology-domain`

第一批任务：

- 定义第一版领域对象
- 定义枚举和值对象
- 定义 ingest / candidate / resolution / explain 类型
- 定义 read-model DTO

完成标准：

- 其他 crate 不再自行定义领域语义

## 5.2 `topology-storage`

第一批任务：

- 落 migration
- 实现 repository trait
- 实现 Postgres repository
- 提供查询组装所需基础查询

完成标准：

- 所有主写路径与查询路径都能经由 storage 层访问数据库

## 5.3 `topology-api`

第一批任务：

- ingest submit handler
- catalog query handler
- topology view handler
- governance/explain handler

完成标准：

- 最小 API 可用于导入、查询、解释

## 5.4 `topology-sync`

第一批任务：

- connector trait
- sync runner
- cursor repository 适配
- CMDB/LDAP 第一批 connector

完成标准：

- 能独立运行同步任务，不耦合到 API 请求线程

## 5.5 `topology-app`

第一批任务：

- 配置加载
- 组件装配
- 单体模式启动
- 角色模式切换

完成标准：

- 本地和测试环境可通过统一入口启动

---

## 6. 里程碑与验收

### Milestone 1：存储基座可用

完成条件：

- `WP-01`
- `WP-02`

验收结果：

- 核心对象表已落地
- migration 可运行
- 基础 repository 可用

### Milestone 2：最小写路径闭环

完成条件：

- `WP-03`

验收结果：

- 业务目录导入可用
- 主机目录导入可用
- 责任关系导入可用

### Milestone 3：最小查询闭环

完成条件：

- `WP-04`
- `WP-06`

验收结果：

- 核心对象可查询
- 基础 topology view 可查询
- 服务可启动并本地演示

### Milestone 4：同步闭环

完成条件：

- `WP-05`
- `WP-07` 部分能力

验收结果：

- 第一批 connector 可同步
- cursor 与重试语义稳定

### Milestone 5：第一版可演示

完成条件：

- `WP-08`
- `WP-07`

验收结果：

- 文档、样例、接口、演示数据齐备
- 可供内部联调或评审演示

---

## 7. 周计划建议

以下计划按 2-4 人小团队估算。

### Week 1

- 完成 `WP-01`
- 确定 migration 方案
- 开始 `WP-02`

### Week 2

- 完成 `WP-02`
- 建立基础存储测试

### Week 3

- 开始 `WP-03`
- 打通业务目录导入

### Week 4

- 完成 `WP-03`
- 打通主机与责任关系导入

### Week 5

- 完成 `WP-04`
- 完成 `WP-06` 的单体模式启动

### Week 6

- 开始 `WP-05`
- 建立 CMDB 或 LDAP 第一条同步链路

### Week 7

- 完成 `WP-05`
- 完成 `WP-07` 的关键日志、指标和审计

### Week 8

- 完成 `WP-08`
- 做一轮端到端联调和演示准备

---

## 8. 风险与控制点

第一版最容易失控的点有四个：

### 8.1 模型范围蔓延

风险：

- 还没打通最小闭环就不断新增对象域

控制：

- 严格限制第一版对象范围
- 新对象必须说明是否阻塞当前里程碑

### 8.2 identity resolution 过早复杂化

风险：

- 在规则和算法上花过多时间，导致主链路迟迟不能落地

控制：

- 第一版优先 deterministic rule
- 冲突先显式暴露，不强行自动裁决

### 8.3 Query API 直接泄漏底表

风险：

- 为了赶进度让 API 直接暴露数据库结构

控制：

- 所有对外返回先定义 DTO
- 视图查询统一走组装层

### 8.4 sync 污染主目录

风险：

- connector 错误、脏数据或游标推进错误导致主对象污染

控制：

- staged payload
- cursor 延后推进
- 同步失败隔离

---

## 9. 当前建议

当前建议固定为：

- 开发管理按工作包和里程碑推进，不按“想到什么做什么”推进
- 每阶段结束都要交付可运行结果，而不是只交付代码片段
- 若资源有限，优先保证 `WP-01` 到 `WP-04`，这是第一版最小闭环
