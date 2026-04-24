pub mod sql {
    pub const UPSERT_BUSINESS: &str = r#"
INSERT INTO business_domain (
    business_id, tenant_id, name, description, created_at, updated_at
) VALUES ($1, $2, $3, $4, $5, $6)
ON CONFLICT (business_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    updated_at = EXCLUDED.updated_at
"#;

    pub const GET_BUSINESS: &str = r#"
SELECT business_id, tenant_id, name, description, created_at, updated_at
FROM business_domain
WHERE business_id = $1
"#;

    pub const LIST_BUSINESSES: &str = r#"
SELECT business_id, tenant_id, name, description, created_at, updated_at
FROM business_domain
WHERE tenant_id = $1
ORDER BY name ASC, business_id ASC
LIMIT $2 OFFSET $3
"#;

    pub const UPSERT_SYSTEM: &str = r#"
INSERT INTO system_boundary (
    system_id, tenant_id, business_id, name, description, created_at, updated_at
) VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT (system_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    business_id = EXCLUDED.business_id,
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    updated_at = EXCLUDED.updated_at
"#;

    pub const UPSERT_SUBSYSTEM: &str = r#"
INSERT INTO subsystem (
    subsystem_id, tenant_id, system_id, name, description, created_at, updated_at
) VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT (subsystem_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    system_id = EXCLUDED.system_id,
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    updated_at = EXCLUDED.updated_at
"#;

    pub const UPSERT_SERVICE: &str = r#"
INSERT INTO service_entity (
    service_id, tenant_id, business_id, system_id, subsystem_id, name, namespace,
    service_type, boundary, provider, external_ref, created_at, updated_at
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
ON CONFLICT (service_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    business_id = EXCLUDED.business_id,
    system_id = EXCLUDED.system_id,
    subsystem_id = EXCLUDED.subsystem_id,
    name = EXCLUDED.name,
    namespace = EXCLUDED.namespace,
    service_type = EXCLUDED.service_type,
    boundary = EXCLUDED.boundary,
    provider = EXCLUDED.provider,
    external_ref = EXCLUDED.external_ref,
    updated_at = EXCLUDED.updated_at
"#;

    pub const GET_SERVICE: &str = r#"
SELECT service_id, tenant_id, business_id, system_id, subsystem_id, name, namespace,
       service_type, boundary, provider, external_ref, created_at, updated_at
FROM service_entity
WHERE service_id = $1
"#;

    pub const LIST_SERVICES: &str = r#"
SELECT service_id, tenant_id, business_id, system_id, subsystem_id, name, namespace,
       service_type, boundary, provider, external_ref, created_at, updated_at
FROM service_entity
WHERE tenant_id = $1
ORDER BY name ASC, service_id ASC
LIMIT $2 OFFSET $3
"#;

    pub const UPSERT_CLUSTER: &str = r#"
INSERT INTO cluster_inventory (
    cluster_id, tenant_id, environment_id, name, provider, created_at, updated_at
) VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT (cluster_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    environment_id = EXCLUDED.environment_id,
    name = EXCLUDED.name,
    provider = EXCLUDED.provider,
    updated_at = EXCLUDED.updated_at
"#;

    pub const UPSERT_NAMESPACE: &str = r#"
INSERT INTO namespace_inventory (
    namespace_id, tenant_id, cluster_id, name, environment_id, created_at, updated_at
) VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT (namespace_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    cluster_id = EXCLUDED.cluster_id,
    name = EXCLUDED.name,
    environment_id = EXCLUDED.environment_id,
    updated_at = EXCLUDED.updated_at
"#;

    pub const UPSERT_WORKLOAD: &str = r#"
INSERT INTO workload_entity (
    workload_id, tenant_id, cluster_id, namespace_id, service_id, kind, name, created_at, updated_at
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
ON CONFLICT (workload_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    cluster_id = EXCLUDED.cluster_id,
    namespace_id = EXCLUDED.namespace_id,
    service_id = EXCLUDED.service_id,
    kind = EXCLUDED.kind,
    name = EXCLUDED.name,
    updated_at = EXCLUDED.updated_at
"#;

    pub const UPSERT_POD: &str = r#"
INSERT INTO pod_inventory (
    pod_id, tenant_id, cluster_id, namespace_id, workload_id, pod_uid, pod_name, created_at, updated_at
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
ON CONFLICT (pod_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    cluster_id = EXCLUDED.cluster_id,
    namespace_id = EXCLUDED.namespace_id,
    workload_id = EXCLUDED.workload_id,
    pod_uid = EXCLUDED.pod_uid,
    pod_name = EXCLUDED.pod_name,
    updated_at = EXCLUDED.updated_at
"#;

    pub const UPSERT_HOST: &str = r#"
INSERT INTO host_inventory (
    host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
    created_at, last_inventory_at
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
ON CONFLICT (host_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    environment_id = EXCLUDED.environment_id,
    host_name = EXCLUDED.host_name,
    machine_id = EXCLUDED.machine_id,
    os_name = EXCLUDED.os_name,
    os_version = EXCLUDED.os_version,
    last_inventory_at = EXCLUDED.last_inventory_at
"#;

    pub const GET_HOST: &str = r#"
SELECT host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
       created_at, last_inventory_at
FROM host_inventory
WHERE host_id = $1
"#;

    pub const LIST_HOSTS: &str = r#"
SELECT host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
       created_at, last_inventory_at
FROM host_inventory
WHERE tenant_id = $1
ORDER BY host_name ASC, host_id ASC
LIMIT $2 OFFSET $3
"#;

    pub const UPSERT_SUBJECT: &str = r#"
INSERT INTO subject (
    subject_id, tenant_id, subject_type, display_name, external_ref, email, is_active,
    created_at, updated_at
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
ON CONFLICT (subject_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    subject_type = EXCLUDED.subject_type,
    display_name = EXCLUDED.display_name,
    external_ref = EXCLUDED.external_ref,
    email = EXCLUDED.email,
    is_active = EXCLUDED.is_active,
    updated_at = EXCLUDED.updated_at
"#;

    pub const INSERT_HOST_RUNTIME_STATE: &str = r#"
INSERT INTO host_runtime_state (
    host_id, observed_at, boot_id, uptime_seconds, loadavg_1m, loadavg_5m, loadavg_15m,
    cpu_usage_pct, memory_used_bytes, memory_available_bytes, process_count, container_count,
    agent_health
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
ON CONFLICT (host_id, observed_at) DO UPDATE SET
    boot_id = EXCLUDED.boot_id,
    uptime_seconds = EXCLUDED.uptime_seconds,
    loadavg_1m = EXCLUDED.loadavg_1m,
    loadavg_5m = EXCLUDED.loadavg_5m,
    loadavg_15m = EXCLUDED.loadavg_15m,
    cpu_usage_pct = EXCLUDED.cpu_usage_pct,
    memory_used_bytes = EXCLUDED.memory_used_bytes,
    memory_available_bytes = EXCLUDED.memory_available_bytes,
    process_count = EXCLUDED.process_count,
    container_count = EXCLUDED.container_count,
    agent_health = EXCLUDED.agent_health
"#;

    pub const UPSERT_SERVICE_INSTANCE: &str = r#"
INSERT INTO service_instance (
    instance_id, tenant_id, service_id, workload_id, started_at, ended_at, last_seen_at
) VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT (instance_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    service_id = EXCLUDED.service_id,
    workload_id = EXCLUDED.workload_id,
    ended_at = EXCLUDED.ended_at,
    last_seen_at = EXCLUDED.last_seen_at
"#;

    pub const UPSERT_RUNTIME_BINDING: &str = r#"
INSERT INTO runtime_binding (
    binding_id, instance_id, object_type, object_id, scope, confidence, source,
    valid_from, valid_to, created_at, updated_at
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
ON CONFLICT (binding_id) DO UPDATE SET
    instance_id = EXCLUDED.instance_id,
    object_type = EXCLUDED.object_type,
    object_id = EXCLUDED.object_id,
    scope = EXCLUDED.scope,
    confidence = EXCLUDED.confidence,
    source = EXCLUDED.source,
    valid_from = EXCLUDED.valid_from,
    valid_to = EXCLUDED.valid_to,
    updated_at = EXCLUDED.updated_at
"#;

    pub const UPSERT_RESPONSIBILITY_ASSIGNMENT: &str = r#"
INSERT INTO responsibility_assignment (
    assignment_id, tenant_id, subject_id, target_kind, target_id, role, source,
    valid_from, valid_to, created_at, updated_at
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
ON CONFLICT (assignment_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    subject_id = EXCLUDED.subject_id,
    target_kind = EXCLUDED.target_kind,
    target_id = EXCLUDED.target_id,
    role = EXCLUDED.role,
    source = EXCLUDED.source,
    valid_from = EXCLUDED.valid_from,
    valid_to = EXCLUDED.valid_to,
    updated_at = EXCLUDED.updated_at
"#;

    pub const UPSERT_EXTERNAL_IDENTITY_LINK: &str = r#"
INSERT INTO external_identity_link (
    link_id, tenant_id, system_type, object_type, external_id, external_key,
    internal_kind, internal_id, status, first_seen_at, last_seen_at, last_synced_at
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
ON CONFLICT (tenant_id, system_type, object_type, external_id) DO UPDATE SET
    external_key = EXCLUDED.external_key,
    internal_kind = EXCLUDED.internal_kind,
    internal_id = EXCLUDED.internal_id,
    status = EXCLUDED.status,
    last_seen_at = EXCLUDED.last_seen_at,
    last_synced_at = EXCLUDED.last_synced_at
"#;

    pub const UPSERT_EXTERNAL_SYNC_CURSOR: &str = r#"
INSERT INTO external_sync_cursor (
    cursor_id, tenant_id, system_type, scope_key, cursor_value, full_sync_token,
    last_success_at, last_attempt_at, last_error, updated_at
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
ON CONFLICT (tenant_id, system_type, scope_key) DO UPDATE SET
    cursor_value = EXCLUDED.cursor_value,
    full_sync_token = EXCLUDED.full_sync_token,
    last_success_at = EXCLUDED.last_success_at,
    last_attempt_at = EXCLUDED.last_attempt_at,
    last_error = EXCLUDED.last_error,
    updated_at = EXCLUDED.updated_at
"#;
}

#[cfg(test)]
mod tests {
    use super::sql;

    #[test]
    fn core_upserts_are_idempotent() {
        for statement in [
            sql::UPSERT_BUSINESS,
            sql::UPSERT_SERVICE,
            sql::UPSERT_HOST,
            sql::UPSERT_SUBJECT,
            sql::UPSERT_RUNTIME_BINDING,
            sql::UPSERT_RESPONSIBILITY_ASSIGNMENT,
            sql::UPSERT_EXTERNAL_IDENTITY_LINK,
            sql::UPSERT_EXTERNAL_SYNC_CURSOR,
        ] {
            assert!(
                statement.contains("ON CONFLICT"),
                "upsert statement must be idempotent: {statement}"
            );
        }
    }

    #[test]
    fn core_queries_include_pagination_where_expected() {
        for statement in [sql::LIST_BUSINESSES, sql::LIST_SERVICES, sql::LIST_HOSTS] {
            assert!(statement.contains("LIMIT $2 OFFSET $3"));
        }
    }
}
