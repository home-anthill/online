use std::env;
use std::sync::OnceLock;

use futures::StreamExt;
use rocket_db_pools::deadpool_redis::redis::{AsyncCommands, aio::MultiplexedConnection};
use subtle::ConstantTimeEq;
use tracing::error;

use crate::errors::db_error::DbError;

static IS_TESTING: OnceLock<bool> = OnceLock::new();

fn is_testing() -> bool {
    *IS_TESTING.get_or_init(|| env::var("ENV").ok().as_deref() == Some("testing"))
}

pub async fn update_notification_silenced(
    db: &mut MultiplexedConnection,
    device_uuid: &str,
    feature_uuid: &str,
    notification_silenced: bool,
) -> Result<(), DbError> {
    let prefix = if is_testing() { "test-alarm-settings" } else { "alarm-settings" };
    let key = format!("{prefix}:{device_uuid}:{feature_uuid}");
    let value = if notification_silenced { "true" } else { "false" };
    db.hset::<_, _, _, ()>(&key, "notificationSilenced", value).await.map_err(|e| {
        error!(target: "app", "update_notification_silenced - Failed to update alarm setting {}: {}", key, e);
        DbError::DbScanError
    })
}

pub async fn update_pending_alarm_api_token(
    db: &mut MultiplexedConnection,
    old_api_token: &str,
    new_api_token: &str,
) -> Result<(), DbError> {
    if old_api_token == new_api_token {
        return Ok(());
    }

    let pattern = if is_testing() { "test-alarm:*" } else { "alarm:*" };
    let keys: Vec<String> = db
        .scan_match::<&str, String>(pattern)
        .await
        .map_err(|e| {
            error!(target: "app", "update_pending_alarm_api_token - Failed to scan alarm events: {}", e);
            DbError::DbScanError
        })?
        .collect()
        .await;

    for key in keys {
        let api_token: Option<String> = db.hget(&key, "apiToken").await.map_err(|e| {
            error!(target: "app", "update_pending_alarm_api_token - Failed to read {}: {}", key, e);
            DbError::DbScanError
        })?;
        // match tokens using in-memory constant-time comparison for security reasons
        let tokens_match: bool =
            api_token.as_deref().map(|token| token.as_bytes().ct_eq(old_api_token.as_bytes()).into()).unwrap_or(false);
        if tokens_match {
            db.hset::<_, _, _, ()>(&key, "apiToken", new_api_token).await.map_err(|e| {
                error!(target: "app", "update_pending_alarm_api_token - Failed to update {}: {}", key, e);
                DbError::DbScanError
            })?;
        }
    }
    Ok(())
}
