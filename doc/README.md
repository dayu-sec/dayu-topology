# dayu-topology 文档索引

## 目录说明

- `architecture/`
  项目定位、边界、模块拆分、存储与服务边界
- `model/`
  核心对象模型、关系模型、拓扑模型与治理模型
- `roadmap/`
  迭代计划、阶段目标、落地顺序

## 建议阅读顺序

1. [glossary.md](./glossary.md)
2. [architecture/project-charter.md](./architecture/project-charter.md)
3. [architecture/system-architecture.md](./architecture/system-architecture.md)
4. [architecture/visual-architecture-guide.md](./architecture/visual-architecture-guide.md)
5. [architecture/storage-architecture.md](./architecture/storage-architecture.md)
6. [architecture/service-and-deployment-architecture.md](./architecture/service-and-deployment-architecture.md)
7. [architecture/ingest-and-normalization-architecture.md](./architecture/ingest-and-normalization-architecture.md)
8. [architecture/external-sync-architecture.md](./architecture/external-sync-architecture.md)
9. [architecture/query-and-read-model-architecture.md](./architecture/query-and-read-model-architecture.md)
10. [architecture/identity-resolution-architecture.md](./architecture/identity-resolution-architecture.md)
11. [architecture/security-and-access-control-architecture.md](./architecture/security-and-access-control-architecture.md)
12. [architecture/observability-and-audit-architecture.md](./architecture/observability-and-audit-architecture.md)
13. [architecture/dataflow-and-pipeline-architecture.md](./architecture/dataflow-and-pipeline-architecture.md)
14. [architecture/unified-model-overview.md](./architecture/unified-model-overview.md)
15. [model/README.md](./model/README.md)
16. [model/unified-topology-schema.md](./model/unified-topology-schema.md)
17. [roadmap/bootstrap-plan.md](./roadmap/bootstrap-plan.md)
18. [roadmap/development-plan.md](./roadmap/development-plan.md)
19. [roadmap/execution-plan.md](./roadmap/execution-plan.md)
20. [roadmap/todo-backlog.md](./roadmap/todo-backlog.md)

## Glossary 同步

当 `doc/glossary.md` 中的标准术语表发生变化后，可运行：

```bash
python3 scripts/sync_glossary.py
```

用途：

- 把 glossary 中的标准术语中英说明同步到带有 `GLOSSARY_SYNC` 标记的文档
- 避免多个模型文档各自维护一份术语解释
- 让 glossary 成为单一术语源
