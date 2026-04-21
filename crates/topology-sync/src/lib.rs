use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalIdentityLink {
    pub link_id: Uuid,
    pub system_type: String,
    pub external_id: String,
    pub internal_id: Uuid,
    pub last_synced_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalSyncCursor {
    pub cursor_id: Uuid,
    pub system_type: String,
    pub scope_key: String,
    pub updated_at: DateTime<Utc>,
}
