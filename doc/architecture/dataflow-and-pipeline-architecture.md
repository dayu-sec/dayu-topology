# dayu-topology 数据流与 Pipeline 架构设计

## 1. 文档目的

本文档定义 `dayu-topology` 第一版主数据流与 pipeline 架构。

目标是固定：

- 数据从哪里来
- 数据由哪些角色生产、同步、归一和消费
- 如何进入中心对象模型
- 哪些阶段做归一化、匹配、派生和落库
- 每类数据构建哪些模型、经过哪些计算过程
- 第一版推荐使用哪些工具、算法和计算策略
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

## 5. 数据生产与计算角色

第一版需要明确“谁生产数据、谁归一数据、谁消费结果”。否则 pipeline 很容易退化成一个不可解释的大 ETL。

### 5.1 数据生产角色

| 角色 | 主要生产数据 | 责任边界 |
| --- | --- | --- |
| Edge Discovery Producer | host、process、container、pod、file、software evidence、security event | 只提供边缘事实和证据，不直接决定中心主键 |
| Telemetry Summarizer | trace 摘要、access log 摘要、network flow 摘要、metrics 窗口摘要 | 把高频 telemetry 压缩成 observation，不把原始明细塞进 topology 主库 |
| External Sync Worker | CMDB、LDAP/IAM/HR、Oncall、漏洞源、服务目录 | 同步外部事实、游标和外部 ID，不直接绕过 normalization 写主模型 |
| Manual / Batch Curator | 人工业务目录、服务依赖声明、责任关系、修正记录 | 提供人工高置信输入，但仍需审计、版本和来源 |
| Security / Risk Producer | EDR、扫描器、制品验真、恶意脚本检测、漏洞命中 | 生产安全证据和风险候选，不直接写最终业务风险结论 |

### 5.2 中心计算角色

| 角色 | 输入 | 输出 | 责任边界 |
| --- | --- | --- | --- |
| Intake Gateway | 原始 payload / batch / stream | `IngestEnvelope` | 负责鉴权、幂等、基础校验和原始载荷归档 |
| Parser / Validator | `IngestEnvelope` | typed raw event / dead letter | 负责 schema 校验、字段检查、时间戳规范化 |
| Candidate Extractor | typed raw event | candidate / evidence / observation | 只抽候选，不决定最终中心身份 |
| Identity Resolver | candidate / external link / catalog | 内部稳定 ID | 做主键解析、去重、冲突识别和置信度计算 |
| Materializer | 已解析 candidate / relation | source-of-truth tables | 幂等写入目录对象、关系对象和运行态对象 |
| Derived View Builder | source-of-truth tables | query view / impact view / explain view | 构建可重算读模型，不作为事实源 |
| Query API | source-of-truth / derived view | API response / graph view | 对外提供稳定查询口径和 explain |

### 5.3 角色边界原则

- 生产者只提供事实和证据，不直接指定中心最终关系。
- Resolver 可以做推断，但必须输出来源、置信度和冲突状态。
- Materializer 只写已解析对象，不能把 unresolved candidate 硬写成正式关系。
- Derived View Builder 可以重算，不能成为唯一 source of truth。
- Query API 不应反向修改主模型。

---

## 6. 按数据来源组织的流程明细

本节按数据来源展开第一版推荐流程。原因是实际接入和排障通常从“这份数据是谁给的、它包含什么、进入后构建哪些模型”开始，而不是先按模型分类。

### 6.1 Edge Discovery 来源

来源角色：

- Edge Discovery Producer

典型数据：

- host discovery
- process facts
- container runtime facts
- pod facts
- file / script facts
- software evidence
- runtime snapshots

| 数据 | 中间层 | 构建模型 | 计算过程 | 推荐工具 / 算法 |
| --- | --- | --- | --- | --- |
| host discovery | `HostCandidate`、`HostEvidence` | `HostInventory` | 解析 `machine_id / hostname / cloud_instance_id`，做 tenant/environment 归属，合并重复主机 | deterministic key matching、source priority、冲突队列 |
| process facts | `ProcessRuntimeCandidate`、`RuntimeBindingCandidate`、`SoftwareEvidence` | `ProcessRuntimeState`、`RuntimeBinding`、`SoftwareEvidence` | 计算启动指纹，关联 host/container，抽取 executable/script evidence，推断 service instance | process start fingerprint、time-window join、rule-based binding、confidence scoring |
| container runtime facts | `ContainerRuntimeCandidate` | `ContainerRuntime`、`RuntimeBinding` | 解析 container id、image、pod、host、namespace/cgroup，绑定到 pod 或 host | cgroup/namespace parsing、container id normalization、join by pod uid |
| pod facts | `PodCandidate`、`PodPlacementEvidence` | `PodInventory`、`PodPlacement` | 解析 cluster/namespace/pod_uid/node，维护 pod 生命周期和 placement 时间段 | deterministic key `(cluster_id,pod_uid)`、valid_from/valid_to interval |
| file / script facts | `SoftwareEvidence` | `SoftwareArtifact(artifact_kind=script)`、`ArtifactVerification` | 计算文件或脚本 `sha256`，记录解释器、shebang、权限、路径和来源 | content hash、interpreter normalization、shebang parser |
| host / process runtime snapshot | `HostRuntimeSnapshot`、`ProcessRuntimeCandidate` | `HostRuntimeState`、`ProcessRuntimeState` | 时间戳归一、单位归一、异常值过滤、按 `observed_at` 写入 | schema validation、unit normalization、MAD / percentile outlier filter |

输出特点：

- Edge 数据是运行事实和证据，不直接等于中心最终关系。
- Edge 最有价值的是稳定标识和证据字段，例如 `machine_id`、`pod_uid`、`container_id`、`process start time`、`sha256`。
- 只要无法解析中心主键，就停留在 candidate / evidence，不写正式关系。

### 6.2 Kubernetes / Runtime API 来源

来源角色：

- K8s Sync
- Runtime API Sync

典型数据：

- cluster / namespace
- workload
- pod owner reference
- service / ingress / endpoint
- label / annotation

| 数据 | 中间层 | 构建模型 | 计算过程 | 推荐工具 / 算法 |
| --- | --- | --- | --- | --- |
| cluster / namespace | `ClusterCandidate`、`NamespaceCandidate` | `ClusterInventory`、`NamespaceInventory` | 解析集群身份、namespace 名称和租户归属 | deterministic external id mapping |
| workload | `WorkloadCandidate` | `WorkloadEntity` | 解析 namespace、workload kind/name、owner reference 和生命周期 | deterministic key `(namespace_id,kind,name)` |
| workload -> pod | `WorkloadPodMembershipCandidate` | `WorkloadPodMembership`、`PodPlacement` | 根据 owner reference、selector、pod_uid 建立 pod 归属 | owner reference matching、label selector matching |
| service / ingress / endpoint | `EndpointCandidate` | `SvcEp`、`EpRes` | 归一 DNS、ClusterIP、Ingress、LB、端口和协议 | DNS canonicalization、endpoint signature hashing |
| labels / annotations | `ServiceWorkloadBindingCandidate`、`RuntimeBindingEvidence` | `ServiceWorkloadBinding`、`RuntimeBindingEvidence` | 从声明或约定解析 service 与 workload 关系 | source priority、rule-based matching、confidence scoring |

输出特点：

- Kubernetes 来源提供强声明关系，通常比流量观测更高置信。
- label / annotation 仍可能脏或缺失，需要保留 evidence 和 confidence。
- workload、pod、service endpoint 是 runtime binding 与 dependency observation 的重要前置数据。

### 6.3 Telemetry Summary 来源

来源角色：

- Telemetry Summarizer

典型数据：

- trace span summary
- access log summary
- gateway log
- DNS log
- network flow summary
- metrics window summary
- error log / crash summary

| 数据 | 中间层 | 构建模型 | 计算过程 | 推荐工具 / 算法 |
| --- | --- | --- | --- | --- |
| trace / access log / flow summary | `DepEv`、`DepObs` | `DepObs`、`EpRes`、`DepEdge` | 地址解析、去噪、窗口聚合、上下游服务解析、生成依赖边候选 | sliding/tumbling window、heavy hitter、health-check filter、DNS cache join |
| DNS / gateway log | `EndpointCandidate`、`DepEv` | `EpRes`、`DepObs`、`DepEdge` | 域名解析为 service/external service，聚合调用方向和目标 | suffix/domain normalization、TTL-aware DNS cache、domain classification |
| metrics window summary | `RuntimeSnapshot`、`HealthCandidate` | `HostRuntimeState`、`BusinessHealthFactor` | 单位归一、趋势/阈值计算、影响 service/business 解析 | percentile、trend detection、threshold evaluation、graph traversal |
| error log / crash summary | `BugEvidence`、`BugObs` | `SoftwareBug`、`SoftwareBugFinding` | 栈归一、错误签名、窗口聚合、版本/制品归因 | stacktrace normalization、fingerprinting、MinHash/SimHash 可选 |

输出特点：

- Telemetry 来源通常是高频数据，必须先摘要、聚合、去噪。
- 单条 telemetry 不直接创建 `DepEdge`、`SoftwareBug` 或业务风险结论。
- 它更适合生产 `DepEv / DepObs / BugObs / HealthCandidate`。

### 6.4 CMDB / 服务目录来源

来源角色：

- External Sync Worker

典型数据：

- business domain
- system / subsystem
- service catalog
- application / host group
- host ownership
- service ownership
- service dependency declaration

| 数据 | 中间层 | 构建模型 | 计算过程 | 推荐工具 / 算法 |
| --- | --- | --- | --- | --- |
| business/system/service catalog | `CatalogCandidate` | `BusinessDomain`、`SystemBoundary`、`Subsystem`、`ServiceEntity` | 名称规范化、层级校验、外部 ID 映射、重复服务归并 | canonical name normalization、external identity link、source priority |
| host / application group | `HostGroupCandidate`、`MembershipCandidate` | `HostGroup`、`HostGroupMembership` | 解析主机组、业务组和成员关系，写入生效时间段 | external id mapping、valid_from/valid_to |
| service workload relation | `ServiceWorkloadBindingCandidate` | `ServiceWorkloadBinding`、`RuntimeBindingEvidence` | 从 CMDB 声明建立 service 与 workload 关系 | source priority、confidence scoring |
| declared dependency | `DeclaredDependencyCandidate` | `DepEdge` | 写入声明性服务依赖，区分 source 和 valid interval | graph edge upsert、valid_from/valid_to |
| responsibility assignment | `ResponsibilityCandidate` | `ResponsibilityAssignment` | 解析业务、服务、主机或组的 owner/maintainer 关系 | source priority、time interval merge |

输出特点：

- CMDB 是强主数据来源，但不应直接作为查询数据库。
- CMDB 中的字符串外键必须通过 `ExternalIdentityLink` 映射到内部稳定主键。
- CMDB 声明关系与 telemetry 观测关系要分开保存，后续可互相校验。

### 6.5 LDAP / IAM / HR 来源

来源角色：

- External Sync Worker

典型数据：

- user
- team
- organization
- membership
- employment status
- disabled/deleted account

| 数据 | 中间层 | 构建模型 | 计算过程 | 推荐工具 / 算法 |
| --- | --- | --- | --- | --- |
| user / team / org | `SubjectCandidate` | `Subject`、`ExternalIdentityLink` | 用户、团队、组织归一，建立外部 ID 到内部 subject 映射 | deterministic external id mapping、source priority |
| membership | `SubjectMembershipCandidate` | `SubjectMembership` 或责任派生视图 | 解析用户与团队、组织的成员关系和有效期 | time range evaluation、valid_from/valid_to |
| employment/account status | `SubjectStatusCandidate` | `Subject` | 更新在职、离职、禁用、删除等状态 | status state machine |

输出特点：

- IAM/HR 负责“人和组织是谁”，不负责最终主机或服务归属。
- 责任关系应通过 `ResponsibilityAssignment` 组合 CMDB、Oncall 和人工修正后生成。

### 6.6 Oncall 来源

来源角色：

- External Sync Worker

典型数据：

- oncall team
- current duty user
- escalation policy
- schedule
- alert route

| 数据 | 中间层 | 构建模型 | 计算过程 | 推荐工具 / 算法 |
| --- | --- | --- | --- | --- |
| schedule / duty user | `OncallCandidate` | effective responsibility view | 解析当前值班人、值班组和时间段 | time range evaluation、priority merge |
| escalation policy | `EscalationCandidate` | responsibility derived view | 解析升级链和告警接收路由 | ordered chain evaluation |
| alert route | `AlertRouteCandidate` | service / host responsibility view | 将告警路由关联到 service、host group 或 business | external identity link、graph traversal |

输出特点：

- Oncall 来源提供“当前谁处理”，不是长期业务归属。
- 它通常进入派生责任视图，不应覆盖 CMDB 的长期 owner/maintainer。

### 6.7 Public Vulnerability / Vendor Advisory 来源

来源角色：

- External Sync Worker

典型数据：

- CVE / GHSA / OSV advisory
- vendor advisory
- affected package / product / version range
- fixed version
- severity

| 数据 | 中间层 | 构建模型 | 计算过程 | 推荐工具 / 算法 |
| --- | --- | --- | --- | --- |
| vulnerability advisory | `VulnRaw`、`FindingCandidate` | `SoftwareVulnerabilityFinding` | advisory 去重、CPE/purl 映射、版本范围求交、finding 幂等写入 | OSV/NVD/vendor advisory parser、semver/rpm/deb version range、interval overlap |
| affected package / version range | `AffectedRangeCandidate` | `SoftwareVersion`、`SoftwareArtifact` 关联 finding | 将外部范围映射到内部 product/version/artifact | purl parser、CPE candidate mapping、version comparator |
| fixed version / severity | `VulnEnrichmentCandidate` | finding enrichment fields | 归一 severity、修复版本、来源可信度 | severity normalization、source priority |

输出特点：

- 漏洞来源只能在完成 software normalization 后产生可靠 finding。
- 外部 `CPE / purl` 不是内部唯一主键，只作为映射线索。

### 6.8 Security / Risk 来源

来源角色：

- Security / Risk Producer

典型数据：

- security event
- EDR alert
- malicious script evidence
- artifact verification result
- weak configuration finding
- intrusion detection event

| 数据 | 中间层 | 构建模型 | 计算过程 | 推荐工具 / 算法 |
| --- | --- | --- | --- | --- |
| security event / EDR alert | `ThreatEvidence`、`RiskCandidate` | `BusinessHealthFactor`、risk derived view | 解析影响对象、严重度归一、关联 host/process/software/service | severity normalization、resource graph traversal、rule scoring |
| malicious script evidence | `SoftwareEvidence`、`ThreatEvidence` | `SoftwareArtifact(artifact_kind=script)`、risk derived view | 脚本 hash、解释器、来源、执行上下文归一 | content hash、interpreter normalization、graph traversal |
| artifact verification result | `ArtifactVerificationCandidate` | `ArtifactVerification` | observed hash、expected hash、签名、包源、镜像 digest 比对 | hash exact match、signature verification、registry metadata lookup |
| weak configuration finding | `RiskCandidate` | `BusinessHealthFactor` 或配置风险视图 | 解析配置项、影响对象、严重度和证据 | rule scoring、policy id mapping |

输出特点：

- 安全来源提供风险证据，不直接写业务最终风险结论。
- 影响面要通过 host/process/software/service/business 图谱计算。

### 6.9 Manual / Batch 来源

来源角色：

- Manual / Batch Curator

典型数据：

- 人工业务目录
- 人工服务依赖
- 人工责任关系
- 批量修正
- 例外和白名单

| 数据 | 中间层 | 构建模型 | 计算过程 | 推荐工具 / 算法 |
| --- | --- | --- | --- | --- |
| manual service catalog | `CatalogCandidate` | `BusinessDomain`、`SystemBoundary`、`ServiceEntity` | 校验层级、名称、外部引用和重复对象 | schema validation、canonical name normalization |
| manual dependency | `DeclaredDependencyCandidate` | `DepEdge` | 写入声明性依赖边，标记人工来源和有效期 | graph edge upsert、valid_from/valid_to |
| manual responsibility | `ResponsibilityCandidate` | `ResponsibilityAssignment` | 写入人工责任关系，保留审批和来源 | audit log、optimistic locking、versioned records |
| correction / override | `CorrectionCandidate` | 对应主对象或关系对象 | 修正错误归属、冲突关系或例外策略 | versioned records、rollback record、approval binding |

输出特点：

- 人工来源通常高置信，但必须可审计、可回滚。
- 人工修正不应删除原始证据，而应以覆盖关系或失效时间表达。

---

## 7. 推荐计算策略与工具

### 7.1 Schema 与数据质量

第一版推荐：

- 使用 JSON Schema / OpenAPI / Protobuf 之一固定外部 envelope 和核心对象契约
- 对批量导入提供 schema validator
- 对关键字段建立数据质量规则

推荐规则：

- 必填字段完整性
- 枚举值合法性
- 时间戳合法性
- tenant / environment 一致性
- 外部 ID 唯一性

### 7.2 Identity Resolution

优先级建议：

1. 确定性 ID 匹配：
   `machine_id`、`pod_uid`、`container_id`、`external_id`、`sha256`
2. 组合键匹配：
   `(tenant_id, host_name)`、`(namespace_id, workload_kind, name)`、`(package_manager, package_name, version)`
3. 规则推断：
   label、命名约定、路径、端口、父子进程
4. 模糊匹配：
   只作为候选，不直接写正式关系

推荐算法：

- deterministic key matching
- source priority merge
- union-find / connected components 用于多源候选合并
- Levenshtein / Jaro-Winkler 用于服务名、团队名的候选提示
- confidence scoring 用于推断绑定

### 7.3 Endpoint Resolution

推荐策略：

- IP/CIDR 用 radix tree 或 CIDR trie
- DNS 结果必须带 TTL 和 `observed_at`
- endpoint signature 应统一包含 protocol、host/ip、port、path scope
- 内外部服务统一用 `ServiceEntity`，用 `boundary` 区分

推荐算法：

- CIDR longest-prefix match
- TTL-aware DNS cache
- endpoint signature hashing
- time-window join

### 7.4 Dependency Observation

推荐策略：

- 单条 trace/log/flow 不直接生成 `DepEdge`
- 先生成 `DepEv`，再窗口聚合成 `DepObs`
- 再由 `DepObs` 支撑 `DepEdge`

推荐算法：

- tumbling / sliding window aggregation
- threshold + confidence scoring
- health-check / sidecar / control-plane traffic filter
- heavy hitter / top-k 聚合
- graph edge upsert with `valid_from / valid_to`

### 7.5 Runtime Binding

推荐策略：

- `ServiceInstance` 作为逻辑服务与运行对象之间的会话锚点
- PID 变化优先表达为 process binding 变化，不直接重建服务
- 声明性绑定优先级高于推断性绑定

推荐算法：

- sessionization
- TTL-based expiry
- start fingerprint
- label selector matching
- parent-child process chain matching
- confidence scoring

### 7.6 Software Normalization

推荐策略：

- `sha256` 是 artifact 级第一优先级证据
- 包管理器和签名用于增强可信度
- `purl` / `CPE` 是外部标识，不作为内部唯一主键

推荐算法 / 工具：

- sha256 exact lookup
- purl parser
- CPE candidate mapping
- semver / rpm / deb version comparator
- package manager database lookup
- container registry metadata lookup
- signature verification

### 7.7 Vulnerability / Bug Matching

推荐策略：

- 漏洞命中必须先完成 software normalization
- 版本范围匹配必须保留来源 advisory 和版本规则
- BUG finding 与 vulnerability finding 分开，安全相关 BUG 再建立关联

推荐算法：

- advisory deduplication by source + advisory id + affected range
- version range intersection
- exact artifact hash match when available
- stacktrace fingerprinting
- MinHash / SimHash 用于相似错误归并，第一版可选

### 7.8 Risk / Impact View

推荐策略：

- 风险视图从 source-of-truth 图谱派生，不直接由告警写死
- 影响面通过 host/process/software/service/business 关系图遍历得到

推荐算法：

- graph traversal
- bounded BFS
- reverse dependency traversal
- weighted risk scoring
- materialized summary view

---

## 8. 数据驱动处理过程

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

### 8.1 Ingest Envelope

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

### 8.2 Parse / Validate

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

### 8.3 Normalize

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

### 8.4 Evidence / Candidate / Observation

不同数据先进入不同中间层。

- `evidence`：证明某个判断的原始事实摘要
- `candidate`：尚未解析完成的候选对象或候选关系
- `observation`：从大量运行数据聚合出的观测摘要

例子：

- IP 扫描结果先进入 `HostNetworkEvidence`
- 访问日志先进入 `DepEv`，聚合后进入 `DepObs`
- 错误日志先进入 bug evidence / bug observation
- 软件路径和 hash 先进入 `SoftwareEvidence`

### 8.5 Identity Resolution

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

### 8.6 Confidence / Conflict

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

### 8.7 Materialize

将解析完成的数据写入 source-of-truth 表。

写入规则：

- 目录对象用 upsert
- 运行态快照按 `observed_at` 写入
- 关系对象按 `valid_from / valid_to` 写入
- evidence / observation 可按保留策略冷热分层
- 派生视图失败不回滚主对象

### 8.8 Derive Views

主模型写入后，再派生查询视图。

典型视图：

- 业务稳定性视图
- 服务依赖图
- 主机 / Pod 拓扑图
- 软件漏洞影响面
- BUG 影响面
- 责任归属视图

---

## 9. 典型输入处理样例

### 9.1 从访问日志生成服务依赖

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

### 9.2 从错误日志发现 BUG

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

### 9.3 从进程发现软件和运行程序真实性

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

### 9.4 从资源指标生成业务健康因子

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

### 9.5 从外部服务访问形成外部依赖

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

## 10. 主数据流

```text
Sources
  -> Intake Envelope
  -> Normalize & Resolve
  -> Write Source of Truth
  -> Build Derived Views
  -> Query / Explain / Export
```

---

## 11. Pipeline 阶段

### 11.1 Source Intake

职责：

- 接收原始输入
- 标记 source、tenant、environment、ingest_time
- 生成稳定 ingest envelope

输出：

- `IngestEnvelope`

### 11.2 Normalize & Resolve

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

### 11.3 Write Source of Truth

职责：

- 幂等 upsert 主表
- 写关系边和时间段
- 写同步游标和外部映射
- 写运行态快照

要求：

- 主写路径必须幂等
- 不应因为派生视图失败而回滚主目录对象

### 11.4 Build Derived Views

职责：

- 生成业务视图
- 生成服务视图
- 生成风险聚合视图
- 生成 explain 视图

说明：

- 这是派生层
- 可重建
- 不应替代 source of truth

### 11.5 Serve Query

职责：

- 面向 API / UI / downstream systems 提供查询
- 返回统一对象视图和关系图
- 支持 explain 查询

---

## 12. 重点 pipeline

### 12.1 资源拓扑 pipeline

```text
edge discovery
  -> host/pod/network facts
  -> normalize
  -> host_inventory / pod_inventory / net_assoc
  -> topology views
```

### 12.2 责任治理 pipeline

```text
cmdb/ldap/oncall
  -> external sync
  -> subject / assignment / link / cursor
  -> effective responsibility view
```

### 12.3 软件安全 pipeline

```text
process/container/package facts
  -> software normalization
  -> software_product / software_version / software_artifact
  -> artifact_verification
  -> vulnerability source ingestion
  -> software_vulnerability_finding
  -> impact view
```

### 12.4 业务稳定性 pipeline

```text
runtime metrics / bug findings / vuln findings / dep observations / threat signals
  -> factor normalization
  -> affected business resolution
  -> business_health_factor
  -> business stability view
```

---

## 13. 关键数据边界

第一版必须固定以下边界：

### 13.1 原始输入与归一对象分开

- intake payload 不是中心主对象

### 13.2 主对象与派生视图分开

- 派生视图失败不应污染主数据

### 13.3 运行态快照与稳定目录对象分开

- `observed_at` 数据不要覆盖 inventory

### 13.4 explain/evidence 与最终结论分开

- `evidence` 支撑结论
- 但不等于最终关系对象本身

### 13.5 候选对象与正式对象分开

- 未解析出内部 ID 的事实不能写正式关系
- 只能进入 evidence / candidate / observation

### 13.6 观测结果与声明事实分开

- 流量观测出的依赖不等于声明依赖
- 漏洞情报命中不等于最终风险结论
- BUG 候选不等于确认 BUG

---

## 14. 第一版失败处理建议

第一版建议：

- intake 失败可重试
- normalize 失败进入死信或人工审查队列
- 主写失败必须显式告警
- 派生视图失败可异步重建

---

## 15. 当前建议

当前建议固定为：

- `dayu-topology` 的 pipeline 设计应以“source of truth 优先”作为原则
- ingest、normalize、persist、derive、query 必须显式分段
- 后续代码实现也应围绕这五段来拆模块
