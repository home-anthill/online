use std::collections::HashMap;
use std::env;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use rocket_db_pools::deadpool_redis::redis::{AsyncCommands, aio::MultiplexedConnection};
use subtle::ConstantTimeEq;
use tracing::{error, info};

use crate::errors::db_error::DbError;
use crate::models::online::Online;

const FCM_BY_API_TOKEN_KEY: &str = "fcm_by_api_token";

static IS_TESTING: OnceLock<bool> = OnceLock::new();

fn is_testing() -> bool {
    *IS_TESTING.get_or_init(|| env::var("ENV").ok().as_deref() == Some("testing"))
}

pub async fn find_all(db: &mut MultiplexedConnection) -> Vec<Online> {
    info!(target: "app", "find_all - To get all online elements from db");

    let mut results: Vec<Online> = Vec::new();

    let db_keys: Vec<String> = match db.scan_match::<&str, String>(get_all_keys_pattern()).await {
        Ok(stream) => stream.collect().await,
        Err(e) => {
            error!(target: "app", "find_all - Failed to scan Redis: {}", e);
            return vec![];
        }
    };

    let key_prefix = if is_testing() { "test_" } else { "online_" };

    for db_key in db_keys {
        let value: HashMap<String, String> = match db.hgetall(&db_key).await {
            Ok(val) => val,
            Err(e) => {
                error!(target: "app", "find_all - Failed to get hash for key {}: {}", db_key, e);
                continue;
            }
        };

        // Key format: {prefix}{device_uuid}_feature_{feature_uuid}
        let Some(without_prefix) = db_key.strip_prefix(key_prefix) else {
            error!(target: "app", "find_all - Unexpected key format: {}", db_key);
            continue;
        };
        let Some((device_uuid, feature_uuid)) = without_prefix.split_once("_feature_") else {
            error!(target: "app", "find_all - Cannot parse device/feature UUIDs from key: {}", db_key);
            continue;
        };

        let api_token = match value.get("apiToken") {
            Some(val) => val.as_str(),
            None => {
                error!(target: "app", "find_all - apiToken is missing for key: {}", db_key);
                continue;
            }
        };
        let fcm_token = value.get("fcmToken").map_or("", |v| v.as_str());

        let created_at = match get_date_field_by_name(&value, "createdAt") {
            Ok(val) => val,
            Err(_) => {
                error!(target: "app", "find_all - cannot parse createdAt for key: {}", db_key);
                continue;
            }
        };
        let modified_at = match get_date_field_by_name(&value, "modifiedAt") {
            Ok(val) => val,
            Err(_) => {
                error!(target: "app", "find_all - cannot parse modifiedAt for key: {}", db_key);
                continue;
            }
        };

        results.push(Online {
            api_token: api_token.to_string(),
            device_uuid: device_uuid.to_string(),
            feature_uuid: feature_uuid.to_string(),
            fcm_token: fcm_token.to_string(),
            created_at: created_at.to_string(),
            modified_at: modified_at.to_string(),
        });
    }
    results
}

pub async fn update_fcm_token_by_api_token(
    db: &mut MultiplexedConnection,
    api_token: &str,
    fcm_token: &str,
) -> Result<(), DbError> {
    if let Err(e) = db.hset::<_, _, _, ()>(FCM_BY_API_TOKEN_KEY, api_token, fcm_token).await {
        error!(target: "app", "update_fcm_token_by_api_token - Failed to cache fcmToken by apiToken: {}", e);
        return Err(DbError::DbScanError);
    }

    let db_keys: Vec<String> = match db.scan_match::<&str, String>(get_all_keys_pattern()).await {
        Ok(stream) => stream.collect().await,
        Err(e) => {
            error!(target: "app", "update_fcm_token_by_api_token - Failed to scan Redis: {}", e);
            return Err(DbError::DbScanError);
        }
    };

    for db_key in db_keys {
        let stored_token: Option<String> = match db.hget(&db_key, "apiToken").await {
            Ok(val) => val,
            Err(e) => {
                error!(target: "app", "update_fcm_token_by_api_token - Failed to get apiToken for key {}: {}", db_key, e);
                continue;
            }
        };
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis().to_string();
        let tokens_match: bool =
            stored_token.as_deref().map(|t| t.as_bytes().ct_eq(api_token.as_bytes()).into()).unwrap_or(false);
        if tokens_match
            && let Err(e) = db
                .hset_multiple::<_, _, _, ()>(
                    &db_key,
                    &[("fcmToken", fcm_token), ("fcmTokenTimestamp", timestamp.as_str())],
                )
                .await
        {
            error!(target: "app", "update_fcm_token_by_api_token - Failed to update fcmToken for key {}: {}", db_key, e);
        }
    }
    Ok(())
}

pub async fn rotate_api_token(
    db: &mut MultiplexedConnection,
    old_api_token: &str,
    new_api_token: &str,
    device_features: &[(String, String)],
) -> Result<(), DbError> {
    let fcm_token: Option<String> = match db.hget(FCM_BY_API_TOKEN_KEY, old_api_token).await {
        Ok(val) => val,
        Err(e) => {
            error!(target: "app", "rotate_api_token - Failed to get fcmToken by old apiToken: {}", e);
            return Err(DbError::DbScanError);
        }
    };
    if let Some(fcm_token) = fcm_token {
        if let Err(e) = db.hset::<_, _, _, ()>(FCM_BY_API_TOKEN_KEY, new_api_token, fcm_token).await {
            error!(target: "app", "rotate_api_token - Failed to cache fcmToken by new apiToken: {}", e);
            return Err(DbError::DbScanError);
        }
        if let Err(e) = db.hdel::<_, _, ()>(FCM_BY_API_TOKEN_KEY, old_api_token).await {
            error!(target: "app", "rotate_api_token - Failed to delete old fcmToken apiToken lookup: {}", e);
            return Err(DbError::DbScanError);
        }
    }

    if !device_features.is_empty() {
        for (device_uuid, feature_uuid) in device_features {
            let db_key = from_uuid_to_db_key(device_uuid, feature_uuid);
            let exists: bool = match db.exists(&db_key).await {
                Ok(val) => val,
                Err(e) => {
                    error!(target: "app", "rotate_api_token - Failed to check key {}: {}", db_key, e);
                    return Err(DbError::DbScanError);
                }
            };
            if !exists {
                continue;
            }

            let stored_token: Option<String> = match db.hget(&db_key, "apiToken").await {
                Ok(val) => val,
                Err(e) => {
                    error!(target: "app", "rotate_api_token - Failed to get apiToken for key {}: {}", db_key, e);
                    return Err(DbError::DbScanError);
                }
            };
            let stored_fcm_token: Option<String> = match db.hget(&db_key, "fcmToken").await {
                Ok(val) => val,
                Err(e) => {
                    error!(target: "app", "rotate_api_token - Failed to get fcmToken for key {}: {}", db_key, e);
                    return Err(DbError::DbScanError);
                }
            };

            if let Some(stored_fcm_token) = stored_fcm_token
                && !stored_fcm_token.is_empty()
                && let Err(e) = db.hset::<_, _, _, ()>(FCM_BY_API_TOKEN_KEY, new_api_token, stored_fcm_token).await
            {
                error!(target: "app", "rotate_api_token - Failed to cache fcmToken by new apiToken: {}", e);
                return Err(DbError::DbScanError);
            }
            if let Some(stored_token) = stored_token.as_deref()
                && stored_token != new_api_token
                && let Err(e) = db.hdel::<_, _, ()>(FCM_BY_API_TOKEN_KEY, stored_token).await
            {
                error!(target: "app", "rotate_api_token - Failed to delete stale fcmToken apiToken lookup: {}", e);
                return Err(DbError::DbScanError);
            }
            if let Err(e) = db.hset::<_, _, _, ()>(&db_key, "apiToken", new_api_token).await {
                error!(target: "app", "rotate_api_token - Failed to update apiToken for key {}: {}", db_key, e);
                return Err(DbError::DbScanError);
            }
        }
        return Ok(());
    }

    let db_keys: Vec<String> = match db.scan_match::<&str, String>(get_all_keys_pattern()).await {
        Ok(stream) => stream.collect().await,
        Err(e) => {
            error!(target: "app", "rotate_api_token - Failed to scan Redis: {}", e);
            return Err(DbError::DbScanError);
        }
    };

    for db_key in db_keys {
        let stored_token: Option<String> = match db.hget(&db_key, "apiToken").await {
            Ok(val) => val,
            Err(e) => {
                error!(target: "app", "rotate_api_token - Failed to get apiToken for key {}: {}", db_key, e);
                continue;
            }
        };
        let tokens_match: bool =
            stored_token.as_deref().map(|t| t.as_bytes().ct_eq(old_api_token.as_bytes()).into()).unwrap_or(false);
        if tokens_match && let Err(e) = db.hset::<_, _, _, ()>(&db_key, "apiToken", new_api_token).await {
            error!(target: "app", "rotate_api_token - Failed to update apiToken for key {}: {}", db_key, e);
            return Err(DbError::DbScanError);
        }
    }
    Ok(())
}

pub async fn update_notification_silenced(
    db: &mut MultiplexedConnection,
    device_uuid: &str,
    feature_uuid: &str,
    notification_silenced: bool,
) -> Result<(), DbError> {
    let db_key = from_uuid_to_db_key(device_uuid, feature_uuid);
    let value = if notification_silenced { "true" } else { "false" };

    if let Err(e) = db.hset::<_, _, _, ()>(&db_key, "notificationSilenced", value).await {
        error!(target: "app", "update_notification_silenced - Failed to update key {}: {}", db_key, e);
        return Err(DbError::DbScanError);
    }
    Ok(())
}

pub fn from_uuid_to_db_key(device_uuid: &str, feature_uuid: &str) -> String {
    let prefix = if is_testing() { "test" } else { "online" };
    format!("{}_{}_feature_{}", prefix, device_uuid, feature_uuid)
}

pub fn get_all_keys_pattern() -> &'static str {
    if is_testing() { "test_*" } else { "online_*" }
}

pub fn get_date_field_by_name(value: &HashMap<String, String>, field_name: &str) -> Result<u128, DbError> {
    if field_name != "createdAt" && field_name != "modifiedAt" {
        return Err(DbError::UnknownFieldNameError);
    }
    match value.get(field_name) {
        Some(val) => val.parse::<u128>().map_err(|_| DbError::DbStrToNumError),
        None => Ok(0u128),
    }
}
