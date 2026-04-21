# dayu-topology Endpoint 与 Dependency Observation 子模型设计

## 1. 文档目的

本文档定义 `dayu-topology` 中心侧 `service endpoint`、`instance endpoint` 以及依赖观测对象的子模型。

目标是固定：

- 服务入口地址与实例运行地址如何表达
- 服务依赖如何区分声明关系与观测关系
- 依赖观测证据如何落库
- 这部分如何与已有 `service / instance / runtime binding / network` 模型衔接

相关文档：

- [`glossary.md`](../glossary.md)
- [`business-system-service-topology-model.md`](./business-system-service-topology-model.md)
- [`runtime-binding-model.md`](./runtime-binding-model.md)
- [`host-pod-network-topology-model.md`](./host-pod-network-topology-model.md)
- [`cluster-namespace-workload-topology-model.md`](./cluster-namespace-workload-topology-model.md)

---

## 2. 核心结论

第一版固定以下结论：

- `ServiceEndpoint` 与 `ServiceInstanceEndpoint` 必须分开
- `ServiceDependency` 只能表达“存在依赖”，不能承载所有观测证据细节
- 观测依赖应独立建模为 `DependencyObservation`
- 一条依赖可由多条观测证据支撑，不应直接把流量细节塞进依赖主表
- 依赖观测必须保留时间、来源、方向、置信度和证据语义

一句话说：

- `ServiceEndpoint` 回答“服务通过什么稳定入口被访问”
- `DependencyObservation` 回答“系统为什么认为两个服务当前存在调用关系”

---

## 3. 为什么必须单独建模

如果直接把依赖写成：

```text
ServiceDependency {
  source_ip
  target_ip
  port
  protocol
  trace_id
}
```

或者把 endpoint 写成：

```text
ServiceEntity {
  address
}
```

会出现明显问题：

- 一个服务可有多个稳定入口
- 一个实例也可有多个动态地址
- 一条服务依赖可能由很多次观测共同支撑
- 依赖事实和观测证据不是同一层对象
- 观测细节量大、变化快，不适合塞入主关系表

因此应明确：

- endpoint 是可连接地址对象
- dependency 是关系对象
- observation 是证据对象

---

## 4. 模型定位

这不是新的顶层模型，而是服务拓扑中的“连接与依赖证据”子模型。

它主要回答：

- 服务通过哪些地址暴露出来
- 实例当前通过哪些地址运行
- 哪两个服务之间被观测到存在依赖
- 这种依赖是基于什么证据判定的

---

## 5. 对象模型

### 5.1 `ServiceEndpoint`

沿用已有定义，表示服务稳定入口。

典型示例：

- DNS
- ClusterIP
- Service DNS
- LoadBalancer VIP
- Ingress 域名
- External API hostname

### 5.2 `ServiceInstanceEndpoint`

沿用已有定义，表示实例当前地址。

典型示例：

- Pod IP:Port
- Host IP:Port
- Container IP:Port

### 5.3 `DependencyObservation`

表示一次归一后的依赖观测记录。

建议结构：

```text
DependencyObservation {
  observation_id
  upstream_service_id?
  upstream_instance_id?
  downstream_service_id?
  downstream_instance_id?
  observation_type
  transport_protocol?
  application_protocol?
  endpoint_signature?
  confidence
  source
  first_observed_at
  last_observed_at
  sample_count?
  created_at
  updated_at
}
```

`observation_type` 示例：

- `network_flow`
- `trace_span`
- `access_log`
- `dns_resolution`
- `config_declared`

`source` 示例：

- `otel_trace`
- `envoy_log`
- `ebpf_flow`
- `nginx_access_log`
- `manual_import`

说明：

- `DependencyObservation` 是观测归一后的摘要对象
- 它不等同原始流量或原始 span

### 5.4 `DependencyObservationEvidence`

表示某条观测依赖的具体证据。

建议结构：

```text
DependencyObservationEvidence {
  evidence_id
  observation_id
  evidence_type
  evidence_ref?
  source_address?
  source_port?
  target_address?
  target_port?
  protocol?
  score?
  observed_at
  metadata?
  created_at
}
```

`evidence_type` 示例：

- `trace_edge`
- `flow_tuple`
- `log_request`
- `dns_answer`
- `route_config`

说明：

- 这层是证据层，不一定要长期保留全部明细
- 第一版可保留摘要证据与原始载荷引用

### 5.5 `EndpointResolution`

表示地址如何解析到服务或实例。

建议结构：

```text
EndpointResolution {
  resolution_id
  endpoint_kind
  address
  port?
  resolved_service_id?
  resolved_instance_id?
  resolution_scope
  confidence
  source
  valid_from
  valid_to?
  created_at
  updated_at
}
```

`endpoint_kind` 示例：

- `dns`
- `ip_port`
- `vip`
- `ingress_host`

`resolution_scope` 示例：

- `service`
- `instance`

说明：

- 依赖观测通常先看到地址，再把地址归一到 service / instance
- 这层对象是地址世界和服务世界之间的桥梁

---

## 6. 关系图谱

第一版建议固定以下关系：

```text
ServiceEntity
  -> ServiceEndpoint[]
  -> ServiceDependency[]

ServiceInstance
  -> ServiceInstanceEndpoint[]

EndpointResolution
  -> ServiceEntity / ServiceInstance

DependencyObservation
  -> DependencyObservationEvidence[]
  -> EndpointResolution[]
  -> ServiceDependency?
```

主链路可表达为：

```text
network/log/trace evidence
  -> DependencyObservationEvidence
  -> DependencyObservation
  -> EndpointResolution
  -> ServiceEntity / ServiceInstance
  -> ServiceDependency
```

---

## 7. 声明依赖与观测依赖

### 7.1 声明依赖

来源包括：

- 架构设计文档
- 平台配置
- service mesh route
- 人工导入

特点：

- 稳定性高
- 变化相对慢
- 可作为期望关系

### 7.2 观测依赖

来源包括：

- trace span
- 流量日志
- eBPF 网络流
- DNS 解析日志
- 网关访问日志

特点：

- 变化快
- 可能有噪声
- 可用于验证声明依赖，或发现隐式依赖

结论：

- `ServiceDependency` 建议保留 `dependency_scope = declared / observed`
- 但具体观测证据应落到 `DependencyObservation`

---

## 8. 与现有模型的衔接

### 8.1 与 `ServiceEntity / ServiceInstance` 模型

- `ServiceEndpoint` 继续承载稳定入口
- `ServiceInstanceEndpoint` 继续承载实例地址
- `EndpointResolution` 负责把观测到的地址归一到服务或实例

### 8.2 与 `RuntimeBinding` 模型

- 如果观测先命中进程或 Pod 地址，再经 `RuntimeBinding` 回连到 `ServiceInstance`
- 所以依赖观测与运行绑定应协同使用，而不是互相替代

### 8.3 与 `Host / Pod / Network` 模型

- `PodNetworkAttachment` 和 `HostNetworkAttachment` 表达网络接入事实
- `DependencyObservation` 表达“实际发生了依赖/访问”
- 接入网络不等于一定存在依赖调用

---

## 9. 第一版查询视图建议

### 9.1 Endpoint 视图

从 `ServiceEntity` 或 `ServiceInstance` 出发，展示：

- 稳定入口地址
- 当前实例地址
- 地址解析如何回连到 service / instance

### 9.2 Dependency 视图

从 `ServiceDependency` 出发，展示：

- 声明依赖还是观测依赖
- 最近一次观测时间
- 主要证据来源
- 主要命中端点签名

### 9.3 Observation Explain 视图

单独展示：

- 为什么系统判定 A 依赖 B
- 命中了哪些 trace / flow / log 证据
- 地址是如何解析成服务对象的

---

## 10. PostgreSQL 存储建议

第一版建议增加：

- `dependency_observation`
- `dependency_observation_evidence`
- `endpoint_resolution`

关键约束建议：

- `endpoint_resolution(endpoint_kind, address, port, valid_to)` 建索引
- `dependency_observation(upstream_service_id, downstream_service_id, source, last_observed_at)` 建索引
- `dependency_observation_evidence(observation_id, observed_at)` 建索引

---

## 11. 第一版最小落地范围

当前建议固定为：

- 先支持 `DependencyObservation`
- 先支持 `DependencyObservationEvidence`
- 先支持 `EndpointResolution`
- 继续沿用现有 `ServiceEndpoint` / `ServiceInstanceEndpoint`

第一版先不要一开始做得过重：

- 不必先存全量原始 trace / flow 明细
- 不必先做复杂调用图权重算法
- 不必先做毫秒级实时依赖更新

先把：

- 地址如何归一到 service / instance
- 观测依赖如何落摘要
- 依赖证据如何解释

三件事固定住。

---

## 12. 当前建议

当前建议固定为：

- endpoint、dependency、observation 必须三层分开
- 地址归一化是依赖观测可落地的关键桥梁
- 观测依赖不应直接等于原始日志或 trace
- 后续依赖图、风险传播、故障影响分析都应依赖这层模型
