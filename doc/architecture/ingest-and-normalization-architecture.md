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

---

## 3. Intake Envelope

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

---

## 4. Candidate Extraction

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

## 5. Identity Resolution

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

## 6. Object & Relation Materialization

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

## 7. Normalization Engine 内部分层

第一版建议在 normalize engine 内部分四类 resolver：

### 7.1 Identity Resolver

负责：

- host / service / workload / subject / software identity

### 7.2 Topology Resolver

负责：

- service -> workload
- workload -> pod
- pod -> host
- pod/host -> network

### 7.3 Runtime Resolver

负责：

- service instance 归属
- runtime binding
- endpoint resolution

### 7.4 Security Resolver

负责：

- software normalization
- vulnerability enrichment 接入前置归一

---

## 8. 主写路径原则

第一版建议固定：

- normalize 结果必须可 explain
- identity resolution 失败可部分降级，但不能静默乱归属
- binding / dependency / responsibility 这类高语义关系要保留来源与置信度
- 写路径优先保证幂等和一致性，不优先追求极致吞吐

---

## 9. 当前建议

当前建议固定为：

- intake 与 normalize 必须显式拆层
- normalize engine 应作为系统中最核心的语义层
- 后续代码结构应围绕 envelope、candidate、resolver、materializer 四层展开
