# 核心模型索引

## 统一模型目录

- [unified-topology-schema.md](./unified-topology-schema.md)

## 数据驱动入口

整体模型按“输入数据 -> evidence / candidate / observation -> identity resolution -> source-of-truth 模型 -> 派生视图”的流程建立。

建议先阅读：

- [../external-integration/input-taxonomy-and-style.md](../external-integration/input-taxonomy-and-style.md)
- [../architecture/dataflow-and-pipeline-architecture.md](../architecture/dataflow-and-pipeline-architecture.md)

再按下面模型文档查看各类输入如何落到具体对象。

- [software-normalization-and-vuln-enrichment.md](./software-normalization-and-vuln-enrichment.md)
- [public-vulnerability-source-ingestion.md](./public-vulnerability-source-ingestion.md)
- [host-inventory-and-runtime-state.md](./host-inventory-and-runtime-state.md)
- [host-inventory-and-runtime-state-schema.md](./host-inventory-and-runtime-state-schema.md)
- [host-inventory-and-runtime-state-storage.md](./host-inventory-and-runtime-state-storage.md)
- [host-responsibility-and-maintainer-model.md](./host-responsibility-and-maintainer-model.md)
- [host-responsibility-sync-from-external-systems.md](./host-responsibility-sync-from-external-systems.md)
- [host-pod-network-topology-model.md](./host-pod-network-topology-model.md)
- [host-process-software-vulnerability-graph.md](./host-process-software-vulnerability-graph.md)
- [business-system-service-topology-model.md](./business-system-service-topology-model.md)
- [cluster-namespace-workload-topology-model.md](./cluster-namespace-workload-topology-model.md)
- [runtime-binding-model.md](./runtime-binding-model.md)
- [endpoint-and-dependency-observation-model.md](./endpoint-and-dependency-observation-model.md)

## 分组建议

### 1. 资产与运行模型

- Host Inventory / Runtime State
- Host / Pod / Network Topology
- Business / System / Service Topology

### 2. 软件与安全模型

- Software Normalization
- Public Vulnerability Source Ingestion
- Host / Process / Software / Vulnerability Graph

### 3. 责任与治理模型

- Host Responsibility and Maintainer Model
- Host Responsibility Sync from External Systems

## 建议阅读顺序

1. [../external-integration/input-taxonomy-and-style.md](../external-integration/input-taxonomy-and-style.md)
2. [host-inventory-and-runtime-state.md](./host-inventory-and-runtime-state.md)
3. [host-responsibility-and-maintainer-model.md](./host-responsibility-and-maintainer-model.md)
4. [host-pod-network-topology-model.md](./host-pod-network-topology-model.md)
5. [business-system-service-topology-model.md](./business-system-service-topology-model.md)
6. [cluster-namespace-workload-topology-model.md](./cluster-namespace-workload-topology-model.md)
7. [runtime-binding-model.md](./runtime-binding-model.md)
8. [endpoint-and-dependency-observation-model.md](./endpoint-and-dependency-observation-model.md)
9. [software-normalization-and-vuln-enrichment.md](./software-normalization-and-vuln-enrichment.md)
10. [public-vulnerability-source-ingestion.md](./public-vulnerability-source-ingestion.md)
11. [host-process-software-vulnerability-graph.md](./host-process-software-vulnerability-graph.md)
