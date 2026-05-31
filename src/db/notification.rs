use std::collections::HashMap;

use rocket_db_pools::deadpool_redis::redis::{AsyncCommands, aio::MultiplexedConnection};
use tracing::error;

use crate::errors::db_error::DbError;
use crate::models::notification::ProfileNotification;

pub fn notification_key(id: &str) -> String {
    format!("notification:{id}")
}

pub fn notifications_by_api_token_key(api_token: &str) -> String {
    format!("notifications:by_api_token:{api_token}")
}

pub async fn list_profile_notifications(
    db: &mut MultiplexedConnection,
    api_token: &str,
) -> Result<Vec<ProfileNotification>, DbError> {
    let index_key = notifications_by_api_token_key(api_token);
    let ids: Vec<String> = db.zrevrange(&index_key, 0, -1).await.map_err(|e| {
        error!(target: "app", "list_profile_notifications - Failed to read index {}: {}", index_key, e);
        DbError::DbScanError
    })?;

    let mut notifications = Vec::with_capacity(ids.len());
    for id in ids {
        let key = notification_key(&id);
        let value: HashMap<String, String> = match db.hgetall(&key).await {
            Ok(val) => val,
            Err(e) => {
                error!(target: "app", "list_profile_notifications - Failed to read hash {}: {}", key, e);
                return Err(DbError::DbScanError);
            }
        };
        if value.is_empty() {
            continue;
        }

        let Some(notification) = notification_from_hash(&key, value) else {
            continue;
        };
        notifications.push(notification);
    }

    Ok(notifications)
}

fn notification_from_hash(key: &str, value: HashMap<String, String>) -> Option<ProfileNotification> {
    let id = value.get("id")?.clone();
    let sent_at = parse_u64_field(key, &value, "sentAt")?;
    let title = value.get("title")?.clone();
    let body = value.get("body")?.clone();
    let device_count = parse_u64_field(key, &value, "deviceCount")?;
    let devices = match value.get("devices").map(|devices| serde_json::from_str(devices)) {
        Some(Ok(devices)) => devices,
        Some(Err(e)) => {
            error!(target: "app", "list_profile_notifications - Invalid devices JSON for key {}: {}", key, e);
            return None;
        }
        None => serde_json::Value::Array(vec![]),
    };
    let provider = value.get("provider").cloned().unwrap_or_default();
    let provider_message_id = value.get("providerMessageId").cloned().unwrap_or_default();

    Some(ProfileNotification { id, sent_at, title, body, device_count, devices, provider, provider_message_id })
}

fn parse_u64_field(key: &str, value: &HashMap<String, String>, field_name: &str) -> Option<u64> {
    match value.get(field_name).map(|val| val.parse::<u64>()) {
        Some(Ok(val)) => Some(val),
        Some(Err(e)) => {
            error!(target: "app", "list_profile_notifications - Invalid {} for key {}: {}", field_name, key, e);
            None
        }
        None => {
            error!(target: "app", "list_profile_notifications - Missing {} for key {}", field_name, key);
            None
        }
    }
}
