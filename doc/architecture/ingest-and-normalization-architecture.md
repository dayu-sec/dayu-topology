# dayu-topology Ingest 与 Normalization 架构设计

## 1. 文档目的

本文档定义 `dayu-topology` 第一版 ingest 与 normalization 架构。

目标是固定：

- 各类输入如何进入系统
- intake envelope 如何统一
- normalization engine 如何分阶段执行
- identity resolution、binding、software normalization、endpoint resolution 如何挂入主写路径

相关文档：

- [`../glossary.md`](../glossary.md)
- [`system-architecture.md`](./system-architecture.md)
- [`dataflow-and-pipeline-architecture.md`](./dataflow-and-pipeline-architecture.md)
- [`../model/runtime-binding-model.md`](../model/runtime-binding-model.md)
- [`../model/software-normalization-and-vuln-enrichment.md`](../model/software-normalization-and-vuln-enrichment.md)

---

## 2. 核心结论

第一版建议把 ingest 与 normalization 拆成四步：

- Intake Envelope
- Candidate Extraction
- Identity Resolution
- Object & Relation Materialization

一句话说：

- 原始输入先统一包装
- 再抽候选对象
- 再做主键解析和归属判定
- 最后产出中心对象和关系边

进一步建议固定：

- `dayu-topology` 的主写路径应是 queue-driven ingest，而不是 source-pull model
- 主动消费队列的是 intake consumer / normalize worker，不是一个直接“从队列拼最终 topology”的大引擎
- 队列负责承载多种协议消息与削峰解耦，模型建立由 resolver / materializer 驱动

---

## 3. Queue-driven Intake Model

### 3.1 为什么采用 queue-driven ingest

对于 `dayu-topology` 这类多来源、多协议、跨时间逐步补全对象的系统，第一版更合理的方式是：

- 外部 producer 产出结构化协议消息
- `dayu-topology` 主动消费 canonical ingress queue / topic
- 在中心侧统一做 envelope、candidate、resolver、materializer

原因：

- 输入源很多，协议不一致，需要统一 intake
- 同一对象会被多源逐步补全，不能要求 producer 直接理解中心模型
- 需要重试、回放、死信、削峰和幂等控制
- 需要允许“先有 discovery、后有 k8s、再有 cmdb”的增量建模过程

### 3.2 什么主动消费队列

更准确地说，主动消费队列的是以下写路径角色：

- Intake Consumer
- Parser / Validator
- Candidate Extractor
- Resolver Worker
- Materializer

不建议表述为：

- “topology engine 直接从队列里拿消息并立刻构建最终模型”

更合理的表述是：

- queue 驱动 intake
- resolver 驱动语义建立
- materializer 驱动中心模型落库

### 3.3 接收什么样的队列消息

队列里承载的应是“符合某个协议族的结构化消息”，而不是中心对象。

建议每条消息至少带有这些路由键：

```text
ProtocolMessage {
  protocol_family
  message_kind
  schema_version
  tenant_id
  partition_key
  message_id
  observed_at?
  payload
}
```

说明：

- `protocol_family`
  表示协议族，例如 `edge.discovery`、`k8s.inventory`、`cmdb.catalog`
- `message_kind`
  表示具体消息类型，例如 `snapshot`、`delta`、`summary`
- `schema_version`
  表示消息 schema 版本
- `partition_key`
  用于把同一冲突域消息尽量路由到同一消费序列

### 3.4 从 queue message 到中心模型的流程

推荐固定以下主链路：

1. Intake Consumer 从 queue / topic 拉取协议消息
2. Parser / Validator 按 `(protocol_family, message_kind, schema_version)` 选择对应 parser
3. 通过基础校验后，统一包装成 `IngestEnvelope`
4. Candidate Extractor 从 envelope 中抽取 candidate / evidence / observation
5. Resolver 链路建立中心语义与内部稳定主键
6. Materializer 幂等写入 source-of-truth 对象、关系和运行态表
7. Derived View Builder 再基于主表更新查询视图

### 3.5 为什么队列消息不能直接等于中心对象

第一版应固定：

- 队列消息只是外部事实，不直接等于中心对象
- producer 不负责决定中心主键
- resolver 才负责 identity resolution、binding、ownership 和 dependency 归属
- unresolved 数据必须停留在 candidate / evidence 层，而不是硬写正式关系

### 3.6 Protocol Registry and Queue Partitioning

第一版建议显式维护一份 protocol registry，用于固定：

- 支持哪些 `protocol_family`
- 每个协议族支持哪些 `message_kind`
- 对应使用哪类 parser / validator / candidate extractor
- 该协议应使用什么 `partition_key`

不建议做成：

- 消费端根据 payload 猜协议类型
- 不带 schema version 的模糊路由
- 同一队列中无路由规则地混合消费所有对象冲突域

#### 3.6.1 第一版建议支持的协议族

建议第一版至少固定以下协议族：

| `protocol_family` | `message_kind` 示例 | 主要用途 |
| --- | --- | --- |
| `edge.discovery` | `snapshot` | 边缘 discovery 资源快照 |
| `k8s.inventory` | `snapshot`、`delta` | cluster / namespace / workload / pod / endpoint 声明性库存 |
| `cmdb.catalog` | `snapshot`、`delta` | business / system / service / host group / ownership |
| `iam.subject` | `snapshot`、`delta` | 用户、团队、组织与成员关系 |
| `oncall.schedule` | `snapshot`、`delta` | 值班、升级链和告警路由 |
| `telemetry.dependency` | `summary_window` | trace / access log / flow 摘要 |
| `telemetry.endpoint` | `summary_window` | DNS / gateway / endpoint 解析摘要 |
| `security.software` | `snapshot`、`delta` | software evidence / artifact verification |
| `security.vulnerability` | `snapshot`、`delta` | advisory / finding 输入 |
| `risk.signal` | `summary_window` | 风险候选、健康因子候选 |
| `manual.catalog` | `batch_upsert` | 人工导入的目录、依赖、责任关系 |

说明：

- 协议族是写路径路由键，不等于数据库表名
- 同一协议族可以有多个 `message_kind`
- 每个协议族都必须带明确 `schema_version`

#### 3.6.2 Protocol Registry 的职责

registry 至少应回答这些问题：

- 这条消息是否属于支持的协议族
- 该 `schema_version` 是否仍被接受
- 应进入哪类 parser / validator
- 应抽取哪些 candidate / evidence / observation
- 失败时应进入 dead letter、retry 还是 reject

#### 3.6.3 `partition_key` 的设计原则

`partition_key` 不应按“来源系统”粗暴切分，而应尽量按“对象冲突域”切分。

原因：

- 同一对象可能被多个来源逐步补全
- 如果相关消息被并发 materialize，容易造成 identity link、binding 和 relationship 抖动
- 正确的分区方式应让同一冲突域消息尽量进入同一消费序列

第一版建议：

- `partition_key` 至少包含 `tenant_id`
- 在 `tenant_id` 之下，再按该协议最核心的对象冲突域拼接
- 若一条消息天然覆盖多个对象，应优先按“主对象”或“主归属对象”切分

#### 3.6.4 协议族建议分区规则

| `protocol_family` | 建议 `partition_key` | 说明 |
| --- | --- | --- |
| `edge.discovery` | `tenant_id + host_identity` | 边缘 discovery 的 host/process/container/file 通常围绕单 host 冲突域 |
| `k8s.inventory` | `tenant_id + cluster_id` | cluster 内 namespace/workload/pod/endpoint 关系耦合较强 |
| `cmdb.catalog` | `tenant_id + external_catalog_scope` | 按业务域、系统域或 cmdb object scope 分区 |
| `iam.subject` | `tenant_id + subject_external_ref` | subject identity 冲突域 |
| `oncall.schedule` | `tenant_id + schedule_or_route_ref` | schedule / route 级串行化 |
| `telemetry.dependency` | `tenant_id + caller_service_or_endpoint` | 依赖观测通常围绕调用方聚合 |
| `telemetry.endpoint` | `tenant_id + endpoint_signature` | endpoint resolution 冲突域 |
| `security.software` | `tenant_id + host_identity` 或 `tenant_id + artifact_identity` | 取决于以 host 侧证据还是 artifact 侧证据为主 |
| `security.vulnerability` | `tenant_id + product_or_artifact_identity` | finding 归并冲突域 |
| `risk.signal` | `tenant_id + affected_object_ref` | 风险候选围绕受影响对象聚合 |
| `manual.catalog` | `tenant_id + batch_scope` | 同一批人工导入保持顺序 |

#### 3.6.5 顺序与并发原则

第一版建议固定：

- 同一 `partition_key` 内顺序消费或串行 materialization
- 不同 `partition_key` 之间允许并发
- 同一对象若跨协议共享冲突域，应在 resolver / materializer 层再次做幂等保护
- 队列分区只能降低冲突概率，不能替代数据库约束和幂等写入

#### 3.6.6 死信与不可解析消息

第一版建议：

- 不支持的 `protocol_family` / `schema_version` 直接 reject 或 dead letter
- schema 校验失败进入 dead letter
- 可重试型外部依赖失败进入 retry queue
- 语义未决但结构合法的消息，不应丢弃，应转换成 unresolved candidate / evidence 保留

---

## 4. Intake Envelope

第一版建议统一输入包装：

```text
IngestEnvelope {
  ingest_id
  source_type
  source_name
  tenant_id
  environment_id?
  observed_at?
  received_at
  payload_ref?
  payload_inline?
  metadata
}
```

作用：

- 统一不同来源的输入入口
- 让后续 normalize 不依赖具体传输协议

补充建议字段：

- `protocol_family`
- `message_kind`
- `schema_version`
- `partition_key?`
- `message_id?`

这样 envelope 能显式保留“外部是按什么协议来的”，但内部 normalize 不需要再直接依赖原传输层。

---

## 5. Candidate Extraction

职责：

- 从 envelope 中抽取候选对象和候选关系

例如：

- host candidate
- workload candidate
- runtime binding candidate
- software evidence candidate
- endpoint candidate

说明：

- 这一步还不产出最终中心主键

---

## 6. Identity Resolution

职责：

- 外部 ID 映射到内部稳定主键
- 解决同一对象多源归并问题
- 建立 `ExternalIdentityLink`

第一版重点解决：

- host identity
- service identity
- workload identity
- subject identity
- software identity

---

## 7. Object & Relation Materialization

职责：

- 把候选对象变成中心对象
- 把候选关系变成关系边
- 把观测结果变成运行态对象或 evidence

产物包括：

- 目录对象
- 关系边
- 运行态快照
- explain/evidence 对象

---

## 8. Normalization Engine 内部分层

第一版建议在 normalize engine 内部分四类 resolver：

### 8.1 Identity Resolver

负责：

- host / service / workload / subject / software identity

### 8.2 Topology Resolver

负责：

- service -> workload
- workload -> pod
- pod -> host
- pod/host -> network

### 8.3 Runtime Resolver

负责：

- service instance 归属
- runtime binding
- endpoint resolution

### 8.4 Security Resolver

负责：

- software normalization
- vulnerability enrichment 接入前置归一

---

## 9. 主写路径原则

第一版建议固定：

- normalize 结果必须可 explain
- identity resolution 失败可部分降级，但不能静默乱归属
- binding / dependency / responsibility 这类高语义关系要保留来源与置信度
- 写路径优先保证幂等和一致性，不优先追求极致吞吐
- queue 负责传输和解耦，不负责决定中心对象语义
- 同一冲突域消息应尽量按 `partition_key` 保证顺序消费或串行 materialization

---

## 10. 当前建议

当前建议固定为：

- intake 与 normalize 必须显式拆层
- normalize engine 应作为系统中最核心的语义层
- 后续代码结构应围绕 envelope、candidate、resolver、materializer 四层展开
- 文档层应明确 `dayu-topology` 是 queue-driven ingest system，而不是外部 source 直写中心模型
