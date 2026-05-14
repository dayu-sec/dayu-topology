# warp-insight → warp-parse → dayu-topology 数据流设计

## 1. 目的

本文档记录当前 `warp-insight` 数据经 `warp-parse` 进入 `dayu-topology` 的真实处理流。

它回答三件事：

- `warp-parse` 当前产出了什么 dayu 输入文件
- `dayu-topology` 现在如何导入这些文件
- 当前结果是落内存还是落真实数据库

相关文档：

- [`../../../asset-twins-demo/warp-insight-to-dayu-dataflow.md`](../../../asset-twins-demo/warp-insight-to-dayu-dataflow.md)
- [`warp-insight-adapter-spec.md`](./warp-insight-adapter-spec.md)
- [`file-ingest-spec.md`](./file-ingest-spec.md)
- [`external-input-spec.md`](./external-input-spec.md)

---

## 2. 当前输入文件

`warp-parse` 当前会输出两个 topology 相关文件：

- `asset-twins-demo/warp-parse/data/out_dat/dayu-edge.jsonl`
- `asset-twins-demo/warp-parse/data/out_dat/dayu-telemetry.jsonl`

其中：

- `dayu-edge.jsonl` 当前主要承载 `dayu.in.edge.v1`
  - host fact
  - process fact
  - 后续可扩展 network / container fact
- `dayu-telemetry.jsonl` 当前承载 `dayu.in.telemetry.v1`
  - host metrics
  - process metrics

---

## 3. 当前导入入口

当前正式导入器已经下沉到 `topology-sync`。

CLI 示例：

```bash
cargo run -q -p topology-app -- postgres-live import-jsonl \
  ../asset-twins-demo/warp-parse/data/out_dat/dayu-edge.jsonl \
  ../asset-twins-demo/warp-parse/data/out_dat/dayu-telemetry.jsonl
```

职责分层：

```text
topology-app
  └─ 解析 CLI 参数
  └─ 初始化 store
  └─ 调用 topology-sync

topology-sync
  └─ JsonlImportService
  └─ 多文件、逐行 JSONL replay / import

topology-api
  └─ 提取 candidates
  └─ identity 解析
  └─ materialize

topology-storage
  └─ 持久化对象
```

对应代码：

- CLI 包装层：
  - [topology-app/src/lib.rs](../../../crates/topology-app/src/lib.rs)
- 正式导入器：
  - [topology-sync/src/lib.rs](../../../crates/topology-sync/src/lib.rs)
- ingest/materialize：
  - [topology-api/src/service.rs](../../../crates/topology-api/src/service.rs)
  - [topology-api/src/ingest.rs](../../../crates/topology-api/src/ingest.rs)

---

## 4. 代码层处理流

```text
dayu-edge.jsonl / dayu-telemetry.jsonl
  ↓
topology-sync::JsonlImportService
  - 打开一个或多个 JSONL 文件
  - 逐行读取
  - 解析 JSON
  - JSON -> DayuInputEnvelope -> IngestEnvelope
  ↓
topology-api::TopologyIngestService
  - 记录 ingest job
  - extract host candidates
  - extract network candidates
  - extract process candidates
  - extract host telemetry candidates
  - extract process telemetry candidates
  ↓
materialize
  - HostInventory
  - NetworkSegment
  - HostNetAssoc
  - ProcessRuntimeState
  - HostRuntimeState
  - ServiceInstance / RuntimeBinding
```

---

## 5. 当前 edge 数据如何被处理

### 5.1 host fact

`dayu.in.edge.v1` 的 host payload 当前会被提取为 `HostCandidate`，再 materialize 为：

- `HostInventory`

如果后续有 network fact 引用该 host，还会作为 network / process 的锚点。

### 5.2 network fact

当前代码已支持提取 network candidate：

- `network_interface`
- `host_network`
- `network`
- `ip`
- `ip_address`

materialize 后会写成：

- `NetworkSegment`
- `HostNetAssoc`

但当前本地 `warp-parse` 产物里还没有有效 network fact，所以联调结果里 `networks=0`。

### 5.3 process fact

`dayu.in.edge.v1` 的 process payload 会被提取为 `ProcessRuntimeCandidate`，再 materialize 为：

- `ProcessRuntimeState`

对 `warp-insight` 风格的 process payload，当前支持从下面字段回推出宿主机：

- `process_key`
- `target_ref`
- `external_ref`

也就是说，即使 process payload 没显式写 `host_name/machine_id`，只要值形如：

```text
hostname:<host>:pid:<pid>:...
```

当前也能挂回已有 host。

### 5.4 process -> service 绑定

如果 process fact 带 `service_ref`，当前会额外生成：

- `ServiceInstance`
- `RuntimeBinding`

当前不会做 PID/name-only 的服务推断。

---

## 6. 当前 telemetry 数据如何被处理

### 6.1 host metrics

`dayu.in.telemetry.v1` 的 host metrics 当前会写入：

- `HostRuntimeState`

当前已接入的 host 指标包括：

- `system.target.count`
- `system.load_average.1m`
- `system.load_average.5m`
- `system.load_average.15m`
- `system.memory.used`
- `system.memory.used_bytes`
- `system.memory.available`
- `system.memory.available_bytes`
- `system.container.count`

### 6.2 process metrics

`dayu.in.telemetry.v1` 的 process metrics 当前不会新建进程，而是回写到已存在的 `ProcessRuntimeState`。

当前已接入：

- `process.state`
- `process.memory.rss`

回写字段：

- `ProcessRuntimeState.process_state`
- `ProcessRuntimeState.memory_rss_kib`

这意味着 telemetry replay 必须在对应 process 已存在之后才有意义。也因此当前推荐顺序是：

1. 先 replay `dayu-edge.jsonl`
2. 再 replay `dayu-telemetry.jsonl`

---

## 7. 当前落库范围

当前 replay/import 可能写入以下 topology 对象：

- `HostInventory`
- `NetworkSegment`
- `HostNetAssoc`
- `ProcessRuntimeState`
- `HostRuntimeState`
- `ServiceInstance`
- `RuntimeBinding`

当前还没有正式接通：

- container runtime 持久化

---

## 8. 当前是否落真实数据库

当前默认不是落真实数据库。

### 8.1 默认模式

`topology-app` 默认使用：

- `InMemoryTopologyStore`

所以默认 replay/import 的结果只存在内存中，进程结束即丢失。

如果使用 `postgres-live import-jsonl` / `postgres-live replace-jsonl`，
则结果会直接落到真实 PostgreSQL。

### 8.2 postgres-mock

当前 `postgres-mock` 也不是真实 PostgreSQL，只是：

- `PostgresTopologyStore<MemoryPostgresExecutor>`

它主要用于验证调用路径和 repository 接口形状，不代表已经落正式数据库。

### 8.3 postgres-live

当前 `topology-app` 已支持真实 PostgreSQL 入口：

```bash
cargo run -q -p topology-app -- postgres-live reset-public

cargo run -q -p topology-app -- postgres-live import-jsonl \
  ../asset-twins-demo/warp-parse/data/out_dat/dayu-edge.jsonl \
  ../asset-twins-demo/warp-parse/data/out_dat/dayu-telemetry.jsonl

cargo run -q -p topology-app -- postgres-live replace-jsonl \
  ../asset-twins-demo/warp-parse/data/out_dat/dayu-edge.jsonl \
  ../asset-twins-demo/warp-parse/data/out_dat/dayu-telemetry.jsonl
```

### 8.4 架构状态

虽然当前默认仍不落真实 DB，但链路已经按正式架构拆分：

- `topology-sync`：正式导入器
- `topology-api`：candidate 提取与 materialize
- `topology-storage`：存储 trait 与 backend

当前 repo 还提供了基于 `docker compose` 管理的 PostgreSQL 开发环境：

- [`../../docker-compose.yml`](../../docker-compose.yml)

它负责：

- 启动和管理 `dayu-topology` 所需 PostgreSQL 实例
- 给 `postgres-live` 提供固定开发目标库

---

## 9. 当前本地联调结果

截至 2026-05-13，本地真实联调结果：

- `lines_total=788`
- `lines_ok=788`
- `lines_failed=0`
- `hosts=1`
- `networks=0`
- `processes=780`
- `processes_enriched=2`
- `host_runtimes=1`

解释：

- `networks=0`
  - 当前 `warp-parse` 产物里没有可导入的 network fact
- `processes=780`
  - `dayu-edge.jsonl` 中已有 780 个进程被 materialize
- `processes_enriched=2`
  - `dayu-telemetry.jsonl` 中只有 2 个进程指标成功回写到了已存在的 `ProcessRuntimeState`
- `host_runtimes=1`
  - host metrics 当前只形成了 1 条 `HostRuntimeState`

---

## 10. 当前结论

当前这条链已经打通到：

- `warp-insight` 导出
- `warp-parse` 投影
- `topology-sync` 正式导入
- `topology-api` materialize
- `topology-storage` 内存落地
- `topology-storage` PostgreSQL 落地

所以现在缺的不是“数据流是否存在”，而是：

- network/container 产物是否补齐
- 更细粒度的 import/reset scope 管理
