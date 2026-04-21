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

第一版主要有四类输入源：

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

---

## 4. 主数据流

```text
Sources
  -> Intake Envelope
  -> Normalize & Resolve
  -> Write Source of Truth
  -> Build Derived Views
  -> Query / Explain / Export
```

---

## 5. Pipeline 阶段

### 5.1 Source Intake

职责：

- 接收原始输入
- 标记 source、tenant、environment、ingest_time
- 生成稳定 ingest envelope

输出：

- `IngestEnvelope`

### 5.2 Normalize & Resolve

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

### 5.3 Write Source of Truth

职责：

- 幂等 upsert 主表
- 写关系边和时间段
- 写同步游标和外部映射
- 写运行态快照

要求：

- 主写路径必须幂等
- 不应因为派生视图失败而回滚主目录对象

### 5.4 Build Derived Views

职责：

- 生成业务视图
- 生成服务视图
- 生成风险聚合视图
- 生成 explain 视图

说明：

- 这是派生层
- 可重建
- 不应替代 source of truth

### 5.5 Serve Query

职责：

- 面向 API / UI / downstream systems 提供查询
- 返回统一对象视图和关系图
- 支持 explain 查询

---

## 6. 三条重点 pipeline

### 6.1 资源拓扑 pipeline

```text
edge discovery
  -> host/pod/network facts
  -> normalize
  -> host_inventory / pod_inventory / attachments
  -> topology views
```

### 6.2 责任治理 pipeline

```text
cmdb/ldap/oncall
  -> external sync
  -> subject / assignment / link / cursor
  -> effective responsibility view
```

### 6.3 软件安全 pipeline

```text
process/container/package facts
  -> software normalization
  -> software_entity
  -> vulnerability source ingestion
  -> software_vulnerability_finding
  -> impact view
```

---

## 7. 关键数据边界

第一版必须固定以下边界：

### 7.1 原始输入与归一对象分开

- intake payload 不是中心主对象

### 7.2 主对象与派生视图分开

- 派生视图失败不应污染主数据

### 7.3 运行态快照与稳定目录对象分开

- `observed_at` 数据不要覆盖 inventory

### 7.4 explain/evidence 与最终结论分开

- `evidence` 支撑结论
- 但不等于最终关系对象本身

---

## 8. 第一版失败处理建议

第一版建议：

- intake 失败可重试
- normalize 失败进入死信或人工审查队列
- 主写失败必须显式告警
- 派生视图失败可异步重建

---

## 9. 当前建议

当前建议固定为：

- `dayu-topology` 的 pipeline 设计应以“source of truth 优先”作为原则
- ingest、normalize、persist、derive、query 必须显式分段
- 后续代码实现也应围绕这五段来拆模块
