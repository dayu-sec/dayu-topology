# dayu-topology 统一 Schema 草案

## 1. 文档目的

本文档给出 `dayu-topology` 第一版统一 schema 草案，目标是把当前分散在多份模型文档中的核心对象收敛到一套中心存储视图中。

本文重点回答：

- 哪些表是核心主表
- 哪些表是关系表
- 哪些表是运行态表
- 哪些主键、外键和唯一约束应优先固定

相关文档：

- [`glossary.md`](../glossary.md)
- [`host-inventory-and-runtime-state-schema.md`](./host-inventory-and-runtime-state-schema.md)
- [`host-responsibility-and-maintainer-model.md`](./host-responsibility-and-maintainer-model.md)
- [`host-pod-network-topology-model.md`](./host-pod-network-topology-model.md)
- [`business-system-service-topology-model.md`](./business-system-service-topology-model.md)
- [`cluster-namespace-workload-topology-model.md`](./cluster-namespace-workload-topology-model.md)
- [`runtime-binding-model.md`](./runtime-binding-model.md)
- [`endpoint-and-dependency-observation-model.md`](./endpoint-and-dependency-observation-model.md)
- [`software-normalization-and-vuln-enrichment.md`](./software-normalization-and-vuln-enrichment.md)

---

## 2. 核心结论

第一版统一 schema 建议按四类表组织：

- 目录主表
- 关系边表
- 运行态表
- 治理与同步表

底层统一建议：

- PostgreSQL 作为主库
- `uuid` 作为中心主键
- `valid_from / valid_to` 作为关系与地址的生效时间语义
- `observed_at` 作为运行态快照时间语义

---

## 3. 目录主表

### 3.1 `business_domain`

主键：

- `business_id`

建议唯一约束：

- `(tenant_id, name)`

### 3.2 `system_boundary`

主键：

- `system_id`

建议索引：

- `(business_id)`
- `(tenant_id, name)`

### 3.3 `subsystem`

主键：

- `subsystem_id`

建议索引：

- `(system_id)`

### 3.4 `service_entity`

主键：

- `service_id`

建议索引：

- `(business_id)`
- `(system_id)`
- `(subsystem_id)`
- `(tenant_id, namespace, name)`

### 3.5 `cluster_inventory`

主键：

- `cluster_id`

建议唯一约束：

- `(tenant_id, environment_id, name)`

### 3.6 `namespace_inventory`

主键：

- `namespace_id`

建议唯一约束：

- `(cluster_id, name)`

建议索引：

- `(tenant_id, name)`
- `(cluster_id)`

### 3.7 `workload_entity`

主键：

- `workload_id`

建议唯一约束：

- `(namespace_id, workload_kind, name)`

建议索引：

- `(service_id)`
- `(cluster_id)`
- `(namespace_id)`

### 3.8 `host_inventory`

主键：

- `host_id`

建议索引：

- `(tenant_id, environment_id, host_name)`
- `(machine_id)`
- `(cloud_instance_id)`

### 3.9 `pod_inventory`

主键：

- `pod_id`

建议唯一约束：

- `(tenant_id, environment_id, cluster_id, pod_uid)`

建议索引：

- `(namespace, pod_name)`
- `(node_id)`

### 3.10 `network_domain`

主键：

- `network_domain_id`

### 3.11 `network_segment`

主键：

- `network_segment_id`

建议索引：

- `(network_domain_id)`
- `(cidr)`

### 3.12 `software_entity`

主键：

- `software_id`

建议索引：

- `(normalized_name)`
- `(publisher)`

### 3.13 `subject`

主键：

- `subject_id`

建议索引：

- `(tenant_id, subject_type, name)`
- `(tenant_id, external_ref)`

### 3.14 `host_group`

主键：

- `host_group_id`

建议唯一约束：

- `(tenant_id, environment_id, name)`

---

## 4. 关系边表

### 4.1 `service_dependency`

主键：

- `dependency_id`

建议索引：

- `(upstream_service_id, valid_to)`
- `(downstream_service_id, valid_to)`
- `(dependency_scope, source)`

### 4.2 `service_instance`

主键：

- `service_instance_id`

建议索引：

- `(service_id, last_seen_at desc)`
- `(pod_id)`
- `(process_id)`
- `(host_id)`

### 4.3 `service_workload_binding`

主键：

- `binding_id`

建议索引：

- `(service_id, valid_to)`
- `(workload_id, valid_to)`

### 4.4 `workload_pod_membership`

主键：

- `(workload_id, pod_id, valid_from)`

建议索引：

- `(workload_id, valid_to)`
- `(pod_id, valid_to)`

### 4.5 `service_endpoint`

主键：

- `endpoint_id`

建议索引：

- `(service_id, valid_to)`
- `(endpoint_type, address, port)`

### 4.6 `service_instance_endpoint`

主键：

- `instance_endpoint_id`

建议索引：

- `(service_instance_id, valid_to)`
- `(address, port, valid_to)`

### 4.7 `pod_placement`

主键：

- `placement_id`

建议索引：

- `(pod_id, valid_to)`
- `(host_id, valid_to)`

### 4.8 `pod_network_attachment`

主键：

- `attachment_id`

建议索引：

- `(pod_id, valid_to)`
- `(network_segment_id, valid_to)`
- `(ip_addr, valid_to)`

### 4.9 `host_network_attachment`

主键：

- `attachment_id`

建议索引：

- `(host_id, valid_to)`
- `(network_segment_id, valid_to)`
- `(ip_addr, valid_to)`

### 4.10 `runtime_binding`

主键：

- `binding_id`

建议索引：

- `(service_instance_id, valid_to)`
- `(runtime_object_type, runtime_object_id, valid_to)`
- `(binding_scope, confidence)`

### 4.11 `runtime_binding_evidence`

主键：

- `evidence_id`

建议索引：

- `(binding_id)`
- `(observed_at)`

### 4.12 `endpoint_resolution`

主键：

- `resolution_id`

建议索引：

- `(endpoint_kind, address, port, valid_to)`
- `(resolved_service_id, valid_to)`
- `(resolved_instance_id, valid_to)`

### 4.13 `dependency_observation`

主键：

- `observation_id`

建议索引：

- `(upstream_service_id, downstream_service_id, source, last_observed_at)`
- `(upstream_instance_id, downstream_instance_id, last_observed_at)`
- `(observation_type, confidence)`

### 4.14 `dependency_observation_evidence`

主键：

- `evidence_id`

建议索引：

- `(observation_id, observed_at)`

### 4.15 `software_evidence`

主键：

- `evidence_id`

建议索引：

- `(software_id)`
- `(host_id)`
- `(pod_id)`
- `(process_id)`

### 4.16 `host_group_membership`

主键：

- `(host_id, host_group_id, valid_from)`

建议索引：

- `(host_id, valid_to)`
- `(host_group_id, valid_to)`

### 4.17 `responsibility_assignment`

主键：

- `assignment_id`

建议索引：

- `(tenant_id, target_type, target_id, role, valid_to)`
- `(subject_id, role, valid_to)`

---

## 5. 运行态表

### 5.1 `host_runtime_state`

建议唯一约束：

- `(host_id, observed_at)`

建议索引：

- `(host_id, observed_at desc)`
- `(observed_at desc)`

### 5.2 `process_runtime_state`

建议唯一约束：

- `(host_id, process_key, observed_at)`

建议索引：

- `(host_id, observed_at desc)`
- `(process_key, observed_at desc)`

说明：

- `process_key` 可采用中心定义的稳定进程 identity

### 5.3 `container_runtime`

主键：

- `container_id`

建议索引：

- `(pod_id, last_seen_at desc)`
- `(host_id, last_seen_at desc)`
- `(runtime_type, runtime_namespace)`

### 5.4 `software_vulnerability_finding`

主键：

- `finding_id`

建议索引：

- `(software_id, status)`
- `(severity, published_at desc)`
- `(source, external_id)`

---

## 6. 同步与治理表

### 6.1 `external_identity_link`

主键：

- `link_id`

建议唯一约束：

- `(tenant_id, system_type, object_type, external_id)`

### 6.2 `external_sync_cursor`

主键：

- `cursor_id`

建议唯一约束：

- `(tenant_id, system_type, scope_key)`

---

## 7. 统一时间语义

第一版建议固定三类时间字段：

- `created_at / updated_at`
  - 目录对象维护时间
- `valid_from / valid_to`
  - 关系、地址、归属、生效段时间
- `observed_at`
  - 运行态快照时间

这三类时间不要混用。

---

## 8. 第一版必须固定的外键主线

```text
system_boundary.business_id -> business_domain.business_id
subsystem.system_id -> system_boundary.system_id
service_entity.system_id -> system_boundary.system_id
service_entity.subsystem_id -> subsystem.subsystem_id
namespace_inventory.cluster_id -> cluster_inventory.cluster_id
workload_entity.cluster_id -> cluster_inventory.cluster_id
workload_entity.namespace_id -> namespace_inventory.namespace_id
workload_entity.service_id -> service_entity.service_id
service_instance.service_id -> service_entity.service_id
service_workload_binding.service_id -> service_entity.service_id
service_workload_binding.workload_id -> workload_entity.workload_id
workload_pod_membership.workload_id -> workload_entity.workload_id
workload_pod_membership.pod_id -> pod_inventory.pod_id
service_endpoint.service_id -> service_entity.service_id
service_instance_endpoint.service_instance_id -> service_instance.service_instance_id
runtime_binding.service_instance_id -> service_instance.service_instance_id
runtime_binding_evidence.binding_id -> runtime_binding.binding_id
pod_placement.pod_id -> pod_inventory.pod_id
pod_placement.host_id -> host_inventory.host_id
pod_network_attachment.pod_id -> pod_inventory.pod_id
pod_network_attachment.network_segment_id -> network_segment.network_segment_id
host_network_attachment.host_id -> host_inventory.host_id
host_network_attachment.network_segment_id -> network_segment.network_segment_id
endpoint_resolution.resolved_service_id -> service_entity.service_id
endpoint_resolution.resolved_instance_id -> service_instance.service_instance_id
dependency_observation.upstream_service_id -> service_entity.service_id
dependency_observation.downstream_service_id -> service_entity.service_id
dependency_observation.upstream_instance_id -> service_instance.service_instance_id
dependency_observation.downstream_instance_id -> service_instance.service_instance_id
dependency_observation_evidence.observation_id -> dependency_observation.observation_id
software_evidence.software_id -> software_entity.software_id
responsibility_assignment.subject_id -> subject.subject_id
host_group_membership.host_group_id -> host_group.host_group_id
software_vulnerability_finding.software_id -> software_entity.software_id
```

---

## 9. 第一版不建议过度设计的部分

第一版先不要一开始就做得过重：

- 不必先做图数据库主存储
- 不必先做全对象事件溯源
- 不必先做复杂多租户跨域共享模型
- 不必先做所有对象的通用 EAV 大表

优先固定：

- 主表是谁
- 关系边是谁
- 时间语义是什么
- 唯一键和核心索引是什么

---

## 10. 第一阶段落地优先级

建议按以下批次落表：

### Phase A

- `business_domain`
- `system_boundary`
- `service_entity`
- `host_inventory`
- `host_runtime_state`
- `subject`
- `responsibility_assignment`

### Phase B

- `cluster_inventory`
- `namespace_inventory`
- `workload_entity`
- `pod_inventory`
- `service_workload_binding`
- `workload_pod_membership`
- `pod_placement`
- `pod_network_attachment`
- `host_network_attachment`

### Phase C

- `service_instance`
- `service_endpoint`
- `service_instance_endpoint`
- `container_runtime`
- `runtime_binding`
- `runtime_binding_evidence`

### Phase D

- `software_entity`
- `software_evidence`
- `software_vulnerability_finding`
- `dependency_observation`
- `dependency_observation_evidence`
- `endpoint_resolution`
- `external_identity_link`
- `external_sync_cursor`

## 11. 当前建议

当前建议固定为：

- 先用 PostgreSQL 把统一 schema 主干定住
- 后续 ingest、sync、query 都围绕这套 schema 展开
- 先从主线对象和主线外键开始，不追求第一版一次覆盖所有细节
