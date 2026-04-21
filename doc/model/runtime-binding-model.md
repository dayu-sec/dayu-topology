# dayu-topology Runtime Binding 子模型设计

## 1. 文档目的

本文档定义 `dayu-topology` 中心侧 `process / container / pod / service instance` 之间的绑定子模型。

目标是固定：

- 运行态对象之间的归属链路如何表达
- 一个进程、容器、Pod 如何归属到某个 `ServiceInstance`
- 什么是稳定绑定，什么是推断绑定
- 这部分如何与已有 `service / workload / pod / host` 模型衔接

相关文档：

- [`glossary.md`](../glossary.md)
- [`business-system-service-topology-model.md`](./business-system-service-topology-model.md)
- [`cluster-namespace-workload-topology-model.md`](./cluster-namespace-workload-topology-model.md)
- [`host-pod-network-topology-model.md`](./host-pod-network-topology-model.md)
- [`host-process-software-vulnerability-graph.md`](./host-process-software-vulnerability-graph.md)

---

## 2. 核心结论

第一版固定以下结论：

- `process`、`container`、`pod`、`service instance` 不是同一个对象
- `ServiceInstance` 是逻辑服务与底层运行对象之间的统一桥接点
- 绑定关系应独立建模，不应只靠对象字段塞一个 `service_id`
- 绑定应区分“声明性绑定”和“推断性绑定”
- 绑定必须有来源、时间段和置信度语义

一句话说：

- `ServiceInstance` 回答“哪个服务的哪个运行副本”
- `RuntimeBinding` 回答“某个 process/container/pod 为什么归到这个实例”

---

## 3. 为什么必须单独建模

如果直接写成：

```text
ProcessRuntimeState {
  service_id
}
```

或者：

```text
PodInventory {
  service_id
}
```

会出现明显问题：

- 同一个 service 下可以有多个运行实例
- 一个 Pod 可能承载多个 container
- sidecar、agent、helper 进程不一定属于业务主服务
- 某些绑定只能推断出来，不能当绝对事实
- 绑定关系会变化，需要历史与来源审计

因此应明确：

- `service` 是逻辑定义
- `service instance` 是运行实例定义
- `process / container / pod` 是运行对象
- 归属关系通过独立 binding 表达

---

## 4. 模型定位

这不是新的顶层模型，而是运行层中的绑定子模型。

它主要回答：

- 一个 `ServiceInstance` 对应哪些运行对象
- 一个 `process` / `container` / `pod` 当前属于哪个实例
- 绑定是从哪里来的、可靠程度如何

它是以下两组模型之间的桥：

- `business / system / service / workload`
- `host / pod / process / software`

---

## 5. 对象与关系模型

### 5.1 `ServiceInstance`

沿用已有定义，作为统一运行实例对象。

关键点：

- 它不直接等于某个 Pod 或进程
- 它是业务逻辑实例的中心锚点

### 5.2 `ContainerRuntime`

表示容器运行对象。

建议结构：

```text
ContainerRuntime {
  container_id
  tenant_id
  pod_id?
  host_id
  runtime_type
  runtime_namespace?
  container_name?
  image_ref?
  started_at?
  last_seen_at
}
```

说明：

- `container` 是独立运行对象，不应只作为 `pod` 的附属字段
- 在 Kubernetes 环境中，它通常归属于某个 pod

### 5.3 `RuntimeBinding`

表示某个运行对象绑定到某个 `ServiceInstance`。

建议结构：

```text
RuntimeBinding {
  binding_id
  service_instance_id
  runtime_object_type
  runtime_object_id
  binding_scope
  confidence
  source
  valid_from
  valid_to?
  created_at
  updated_at
}
```

`runtime_object_type` 建议取值：

- `process`
- `container`
- `pod`

`binding_scope` 建议取值：

- `declared`
- `observed`
- `inferred`

`confidence` 建议取值：

- `high`
- `medium`
- `low`

说明：

- 绑定对象统一成一个表，比拆成多张小表更利于统一查询
- 通过 `runtime_object_type + runtime_object_id` 区分对象类型

### 5.4 `RuntimeBindingEvidence`

表示绑定判定依据。

建议结构：

```text
RuntimeBindingEvidence {
  evidence_id
  binding_id
  evidence_type
  evidence_value
  score?
  observed_at?
  created_at
}
```

`evidence_type` 示例：

- `k8s_label_match`
- `pod_owner_match`
- `container_name_match`
- `port_signature_match`
- `binary_path_match`
- `env_var_match`
- `manual_override`

说明：

- 绑定不是黑盒结果，应尽量保留可解释依据
- 这样后续才能调试错误归属和误绑定问题

---

## 6. 主绑定链路

第一版建议固定如下主链路：

### 6.1 Kubernetes 主路径

```text
ServiceEntity
  -> ServiceInstance
  -> RuntimeBinding(type = pod)
  -> PodInventory
  -> PodPlacement
  -> HostInventory
```

然后可选扩展为：

```text
ServiceInstance
  -> RuntimeBinding(type = container)
  -> ContainerRuntime
```

### 6.2 主机进程主路径

```text
ServiceEntity
  -> ServiceInstance
  -> RuntimeBinding(type = process)
  -> ProcessRuntimeState
  -> HostInventory
```

### 6.3 完整细粒度路径

```text
ServiceEntity
  -> ServiceInstance
  -> RuntimeBinding(type = pod)
  -> PodInventory
  -> RuntimeBinding(type = container)
  -> ContainerRuntime
  -> RuntimeBinding(type = process)
  -> ProcessRuntimeState
```

第一版不要求所有路径同时完整实现，但模型上要允许这样展开。

---

## 7. 声明绑定与推断绑定

### 7.1 声明绑定

来源包括：

- deployment / workload 显式 metadata
- service-to-workload 配置
- 手工治理配置
- 平台注册信息

特点：

- 稳定性更高
- 优先级更高

### 7.2 推断绑定

来源包括：

- label / annotation
- container name
- image signature
- port / endpoint 特征
- 可执行路径 / binary name
- telemetry 侧观测特征

特点：

- 适合补全未知归属
- 必须带 `confidence` 和 `evidence`

结论：

- 查询结果可混合两类绑定
- 但系统内部必须能区分它们

---

## 8. 与现有模型的衔接

### 8.1 与 `Business / System / Service` 模型

- `ServiceInstance` 继续承载业务服务的运行实例身份
- 本模型只解决实例和运行对象之间的归属绑定

### 8.2 与 `Cluster / Namespace / Workload` 模型

- `service -> workload -> pod` 仍是主编排路径
- 本模型补 `pod -> service instance` 及细粒度 `container / process` 归属

### 8.3 与 `Host / Pod / Network` 模型

- `PodInventory`、`PodPlacement`、`HostInventory` 继续表达拓扑
- 本模型不替代拓扑关系，只补业务归属关系

### 8.4 与软件和漏洞模型

这层绑定完成后，可以形成：

```text
ProcessRuntimeState
  -> SoftwareEvidence
  -> SoftwareEntity
  -> SoftwareVulnerabilityFinding
  -> ServiceInstance
  -> ServiceEntity
```

这样可以回答：

- 某个漏洞影响了哪个服务实例
- 它是通过哪个进程/容器命中的

---

## 9. 第一版查询视图建议

### 9.1 Service Instance 视图

从 `ServiceInstance` 出发，展示：

- 它归属哪个 `ServiceEntity`
- 绑定了哪些 pod / container / process
- 每条绑定的来源和置信度

### 9.2 Runtime Object 视图

从 `process` / `container` / `pod` 出发，展示：

- 当前归属哪个 `ServiceInstance`
- 是声明绑定还是推断绑定
- 绑定证据是什么

### 9.3 Binding Explain 视图

单独展示：

- 为什么这个运行对象被归到该服务实例
- 证据链和评分是什么
- 是否存在冲突绑定

---

## 10. PostgreSQL 存储建议

第一版建议增加：

- `container_runtime`
- `runtime_binding`
- `runtime_binding_evidence`

关键约束建议：

- `(runtime_object_type, runtime_object_id, valid_to)` 应控制单时刻有效绑定数
- `(service_instance_id, runtime_object_type, runtime_object_id, valid_from)` 可作为幂等落库键候选

建议索引：

- `runtime_binding(service_instance_id, valid_to)`
- `runtime_binding(runtime_object_type, runtime_object_id, valid_to)`
- `runtime_binding(binding_scope, confidence)`
- `runtime_binding_evidence(binding_id)`

---

## 11. 第一版最小落地范围

当前建议固定为：

- 先支持 `RuntimeBinding`
- 先支持 `RuntimeBindingEvidence`
- `container_runtime` 若 discovery 还不完整，可先保留 schema 与占位对象

第一版先不要一开始做得过重：

- 不必先做完美自动归属
- 不必先做所有 sidecar / helper 的精确分类
- 不必先做复杂多来源冲突仲裁引擎

先把：

- pod 能否归属到 service instance
- process 能否归属到 service instance
- 绑定是否可解释

三件事固定住。

---

## 12. 当前建议

当前建议固定为：

- `RuntimeBinding` 是统一运行归属模型中的关键桥梁
- `ServiceInstance` 不应直接等于某个 pod/process/container
- 绑定应保留 `scope / confidence / source / evidence`
- 后续 service 查询、漏洞关联、责任影响分析都应依赖这层绑定模型
