# dayu-topology 数据流与 Pipeline 架构设计

## 1. 文档目的

本文档定义 `dayu-topology` 第一版主数据流与 pipeline 架构。

目标是固定：

- 数据从哪里来
- 如何进入中心对象模型
- 哪些阶段做归一化、匹配、派生和落库
- 哪些结果是 source of truth，哪些结果是读模型

相关文档：

- [`../glossary.md`](../glossary.md)
- [`system-architecture.md`](./system-architecture.md)
- [`unified-model-overview.md`](./unified-model-overview.md)
- [`../model/unified-topology-schema.md`](../model/unified-topology-schema.md)

---

## 2. 核心结论

第一版建议把数据流分成五段：

- Source Intake
- Normalize & Resolve
- Persist Source of Truth
- Build Derived Views
- Serve Query

一句话说：

- 外部事实先进入 intake
- 再做归一与主键解析
- 然后落主库
- 最后投影成可查视图

---

## 3. 输入源分类

第一版主要有以下输入源。

整体原则：

- 输入数据只是事实来源，不直接等于中心模型对象
- 所有输入先进入 evidence / candidate / observation / staging 层
- 经过归一、身份解析、聚合和验真后，才写入主模型
- 无法解析的数据不能硬写正式关系，应保留候选和证据

### 3.1 Edge / Discovery 输入

例如：

- host discovery
- process facts
- pod / container facts
- runtime snapshots

### 3.2 External Sync 输入

例如：

- CMDB
- LDAP / IAM / HR
- Oncall
- 公共漏洞源

### 3.3 Manual / Batch 输入

例如：

- 业务系统目录导入
- 服务依赖定义导入
- 人工责任关系导入

### 3.4 Observed Telemetry-derived 输入

例如：

- trace 边摘要
- access log 摘要
- network flow 摘要

### 3.5 Security / Runtime Risk 输入

例如：

- EDR 告警
- 恶意脚本发现
- 弱配置扫描
- 入侵检测事件
- 运行制品验真结果

### 3.6 Error / Bug 输入

例如：

- 应用错误日志
- crash dump 摘要
- panic / exception stack trace
- 进程异常退出事件
- 用户工单或告警归因结果

---

## 4. 输入数据到模型对象的映射

第一版建议按输入数据类型整理处理过程。

| 输入数据 | 先进入的中间层 | 关键处理 | 最终模型对象 |
| --- | --- | --- | --- |
| 主机发现数据 | inventory candidate | 主机 identity resolution、租户归属、去重 | `HostInventory` |
| 主机运行指标 | runtime snapshot | 时间戳归一、指标标准化、异常值过滤 | `HostRuntimeState` |
| Pod / 容器事实 | inventory / placement evidence | cluster / namespace / workload / pod 解析 | `ClusterInventory`、`NamespaceInventory`、`WorkloadEntity`、`PodInventory`、`PodPlacement` |
| 主机 / Pod 网络事实 | network evidence / candidate | IP、MAC、CIDR、网络段解析 | `HostNetAssoc`、`PodNetAssoc`、`NetworkSegment` |
| 进程事实 | runtime snapshot / binding candidate | 进程 identity、启动指纹、服务归属推断 | `ProcessRuntimeState`、`RuntimeBinding` |
| 容器运行事实 | runtime candidate | container id、image、pod、host 归属解析 | `ContainerRuntime`、`RuntimeBinding` |
| 业务 / 系统 / 服务目录 | catalog candidate | 名称规范化、层级关系解析、外部 ID 映射 | `BusinessDomain`、`SystemBoundary`、`Subsystem`、`ServiceEntity` |
| 外部服务目录 | catalog candidate | provider、external_ref、boundary 识别 | `ServiceEntity(boundary=external/partner/saas)` |
| 服务入口配置 | endpoint candidate | DNS、VIP、Ingress、LB 入口归一 | `SvcEp` |
| 实例监听地址 | endpoint candidate | 实例地址、端口、协议、生命周期解析 | `InstEp` |
| trace / access log / flow | dependency evidence / observation | 解析地址、去噪、窗口聚合、地址归一 | `DepEv`、`DepObs`、`EpRes`、`DepEdge` |
| DNS / 网关日志 | dependency evidence / endpoint candidate | 域名解析、目标服务归一、缓存 | `EpRes`、`DepObs`、`DepEdge` |
| 软件包 / 可执行文件 | software evidence | 产品、版本、制品归一，hash / 签名解析 | `SoftwareProduct`、`SoftwareVersion`、`SoftwareArtifact` |
| 脚本文件 | software evidence | 脚本内容 `sha256`、解释器、权限、来源解析 | `SoftwareArtifact(artifact_kind=script)` |
| 运行程序验真 | verification evidence | observed hash、expected hash、签名、包源、证明校验 | `ArtifactVerification` |
| 漏洞情报 | vuln raw / finding candidate | advisory 去重、版本范围匹配、CPE/purl 映射 | `SoftwareVulnerabilityFinding` |
| 错误日志 / crash | bug evidence / observation | error_signature、窗口聚合、版本/制品归因 | `SoftwareBug`、`SoftwareBugFinding` |
| 责任 / 人员 / 组织 | governance candidate | subject 归一、外部身份映射、时间段解析 | `Subject`、`ResponsibilityAssignment` |
| 安全威胁 / 异常行为 | threat evidence / health candidate | 告警归一、影响对象解析、严重度归一 | `BusinessHealthFactor(factor_type=threat_reduction)` |
| 容量 / 资源风险 | health candidate | 资源水位、趋势、阈值、影响业务解析 | `BusinessHealthFactor(factor_type=resource_sufficiency)` |

---

## 5. 数据驱动处理过程

统一处理过程如下：

```text
input data
  -> ingest envelope
  -> parse / validate
  -> normalize
  -> evidence / candidate / observation
  -> identity resolution
  -> confidence / conflict handling
  -> materialize source-of-truth model
  -> derive health / risk / topology views
```

### 5.1 Ingest Envelope

所有输入先包装成统一 envelope。

建议包含：

- `tenant_id`
- `source`
- `source_type`
- `ingest_id`
- `ingested_at`
- `schema_version`
- `payload_ref`
- `raw_hash`

要求：

- 原始 payload 不直接写入主模型
- 原始数据可放对象存储、日志系统或 staging 表
- 主模型只保存归一结果和必要证据引用

### 5.2 Parse / Validate

处理内容：

- schema 校验
- 必填字段检查
- 时间戳标准化
- 枚举值规范化
- 明显脏数据过滤

失败处理：

- schema 错误进入 dead letter
- 字段缺失进入 candidate / unresolved
- 不因单条失败阻塞整个批次

### 5.3 Normalize

处理内容：

- 名称归一
- 地址归一
- 版本归一
- 路径归一
- 枚举值归一
- source-specific 字段映射到统一字段

示例：

- `serviceName`、`app_name`、`workload` 归一为服务候选名
- `10.0.1.1:8080`、`host:port` 归一为 endpoint candidate
- `v1.2.3-build7` 归一为 `normalized_version`

### 5.4 Evidence / Candidate / Observation

不同数据先进入不同中间层。

- `evidence`：证明某个判断的原始事实摘要
- `candidate`：尚未解析完成的候选对象或候选关系
- `observation`：从大量运行数据聚合出的观测摘要

例子：

- IP 扫描结果先进入 `HostNetworkEvidence`
- 访问日志先进入 `DepEv`，聚合后进入 `DepObs`
- 错误日志先进入 bug evidence / bug observation
- 软件路径和 hash 先进入 `SoftwareEvidence`

### 5.5 Identity Resolution

把外部事实解析成中心 ID。

常见解析目标：

- `host_id`
- `pod_id`
- `workload_id`
- `service_id`
- `inst_id`
- `product_id`
- `version_id`
- `artifact_id`
- `net_seg_id`

规则：

- 解析成功才能写正式主对象或关系
- 多候选冲突时进入 candidate，不强行选择
- 解析过程要保留来源、置信度和证据

### 5.6 Confidence / Conflict

每个推断性结果都应有置信度。

建议等级：

- `high`
- `medium`
- `low`

冲突处理：

- 高置信覆盖低置信
- 同级冲突进入人工审查或冲突队列
- 关系对象通过 `valid_from / valid_to` 保留历史
- 不直接删除旧事实，优先写失效时间

### 5.7 Materialize

将解析完成的数据写入 source-of-truth 表。

写入规则：

- 目录对象用 upsert
- 运行态快照按 `observed_at` 写入
- 关系对象按 `valid_from / valid_to` 写入
- evidence / observation 可按保留策略冷热分层
- 派生视图失败不回滚主对象

### 5.8 Derive Views

主模型写入后，再派生查询视图。

典型视图：

- 业务稳定性视图
- 服务依赖图
- 主机 / Pod 拓扑图
- 软件漏洞影响面
- BUG 影响面
- 责任归属视图

---

## 6. 典型输入处理样例

### 6.1 从访问日志生成服务依赖

```text
access log
  -> parse src/dst/port/protocol/status
  -> DepEv
  -> window aggregate
  -> DepObs
  -> EpRes
  -> DepEdge
```

要点：

- 单条日志不直接创建 `DepEdge`
- 健康检查和 sidecar 控制面流量要过滤
- 地址解析失败时停留在 observation / unresolved bucket

### 6.2 从错误日志发现 BUG

```text
error log / crash dump
  -> normalize stack / error code
  -> compute error_signature
  -> attach product/version/artifact
  -> BugObs
  -> SoftwareBugFinding
  -> SoftwareBug?
```

要点：

- 单条错误日志不直接创建 `SoftwareBug`
- 需要重复出现、签名稳定、能归因到版本或制品
- 环境问题、配置问题、外部依赖不可用要先排除

### 6.3 从进程发现软件和运行程序真实性

```text
process facts
  -> executable path / script path / sha256
  -> SoftwareEvidence
  -> SoftwareArtifact
  -> ArtifactVerification
  -> SoftwareVersion / SoftwareProduct
```

要点：

- 进程名和路径不能证明程序真实
- 可执行文件和脚本优先用 `sha256` 归一
- 签名、包源、镜像和远程证明用于提高可信度

### 6.4 从资源指标生成业务健康因子

```text
host / pod / workload metrics
  -> runtime snapshot
  -> threshold / trend evaluation
  -> affected service / business resolution
  -> BusinessHealthFactor
```

要点：

- 业务健康因子不是原始指标
- 它是资源、BUG、漏洞、依赖、威胁五类信号的摘要
- 原始证据通过 `evidence_ref` 回指

### 6.5 从外部服务访问形成外部依赖

```text
gateway log / dns / access log
  -> endpoint candidate
  -> EpRes
  -> ServiceEntity(boundary=external)
  -> DepEdge
```

要点：

- 外部 API、SaaS、合作方服务复用 `ServiceEntity`
- 不创建内部 `ServiceInstance`
- 依赖边仍然统一用 `DepEdge`

---

## 7. 主数据流

```text
Sources
  -> Intake Envelope
  -> Normalize & Resolve
  -> Write Source of Truth
  -> Build Derived Views
  -> Query / Explain / Export
```

---

## 8. Pipeline 阶段

### 8.1 Source Intake

职责：

- 接收原始输入
- 标记 source、tenant、environment、ingest_time
- 生成稳定 ingest envelope

输出：

- `IngestEnvelope`

### 8.2 Normalize & Resolve

职责：

- identity resolution
- 外部 ID 到内部主键映射
- service / workload / pod / process 归属绑定
- software normalization
- endpoint resolution

输出：

- 目录对象候选
- 关系边候选
- 运行态快照候选
- explain/evidence 候选

### 8.3 Write Source of Truth

职责：

- 幂等 upsert 主表
- 写关系边和时间段
- 写同步游标和外部映射
- 写运行态快照

要求：

- 主写路径必须幂等
- 不应因为派生视图失败而回滚主目录对象

### 8.4 Build Derived Views

职责：

- 生成业务视图
- 生成服务视图
- 生成风险聚合视图
- 生成 explain 视图

说明：

- 这是派生层
- 可重建
- 不应替代 source of truth

### 8.5 Serve Query

职责：

- 面向 API / UI / downstream systems 提供查询
- 返回统一对象视图和关系图
- 支持 explain 查询

---

## 9. 重点 pipeline

### 9.1 资源拓扑 pipeline

```text
edge discovery
  -> host/pod/network facts
  -> normalize
  -> host_inventory / pod_inventory / net_assoc
  -> topology views
```

### 9.2 责任治理 pipeline

```text
cmdb/ldap/oncall
  -> external sync
  -> subject / assignment / link / cursor
  -> effective responsibility view
```

### 9.3 软件安全 pipeline

```text
process/container/package facts
  -> software normalization
  -> software_product / software_version / software_artifact
  -> artifact_verification
  -> vulnerability source ingestion
  -> software_vulnerability_finding
  -> impact view
```

### 9.4 业务稳定性 pipeline

```text
runtime metrics / bug findings / vuln findings / dep observations / threat signals
  -> factor normalization
  -> affected business resolution
  -> business_health_factor
  -> business stability view
```

---

## 10. 关键数据边界

第一版必须固定以下边界：

### 10.1 原始输入与归一对象分开

- intake payload 不是中心主对象

### 10.2 主对象与派生视图分开

- 派生视图失败不应污染主数据

### 10.3 运行态快照与稳定目录对象分开

- `observed_at` 数据不要覆盖 inventory

### 10.4 explain/evidence 与最终结论分开

- `evidence` 支撑结论
- 但不等于最终关系对象本身

### 10.5 候选对象与正式对象分开

- 未解析出内部 ID 的事实不能写正式关系
- 只能进入 evidence / candidate / observation

### 10.6 观测结果与声明事实分开

- 流量观测出的依赖不等于声明依赖
- 漏洞情报命中不等于最终风险结论
- BUG 候选不等于确认 BUG

---

## 11. 第一版失败处理建议

第一版建议：

- intake 失败可重试
- normalize 失败进入死信或人工审查队列
- 主写失败必须显式告警
- 派生视图失败可异步重建

---

## 12. 当前建议

当前建议固定为：

- `dayu-topology` 的 pipeline 设计应以“source of truth 优先”作为原则
- ingest、normalize、persist、derive、query 必须显式分段
- 后续代码实现也应围绕这五段来拆模块
