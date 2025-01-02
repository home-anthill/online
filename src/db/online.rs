use futures::StreamExt;
use log::{error, info};
use std::collections::HashMap;
use std::env;

use rocket_db_pools::deadpool_redis::redis::{aio::MultiplexedConnection, AsyncCommands};

use crate::errors::db_error::DbError;
use crate::models::online::Online;

pub async fn find_all(db: &MultiplexedConnection) -> Vec<Online> {
    info!(target: "app", "find_all - To get all online elements from db");
    let mut con = db.clone();

    let mut not_online_devices: Vec<Online> = vec![];

    // get all keys with format 'online-<uuid>' or 'test-<uuid>'
    let db_keys: Vec<String> = con
        .scan_match::<&str, String>(get_all_keys_pattern().as_str())
        .await
        .unwrap()
        .collect()
        .await;

    for db_key in db_keys {
        // hgetall returns the entire redis hash table (with all "key: value")
        let value: HashMap<String, String> = con.hgetall(db_key.as_str()).await.unwrap();
        let api_token: Result<&str, DbError> = match &value.get("apiToken") {
            Some(val) => Ok(val),
            None => Err(DbError::DbNotFound),
        };
        let fcm_token: Result<&str, DbError> = match &value.get("fcmToken") {
            Some(val) => Ok(val),
            None => Ok(""),
        };

        let created_at: Result<u128, DbError> = get_date_field_by_name(&value, "createdAt");
        let modified_at: Result<u128, DbError> = get_date_field_by_name(&value, "modifiedAt");

        if api_token.is_err() {
            error!(target: "app", "REST - GET - find_all - apiToken is missing");
            continue;
        }
        if created_at.is_err() || modified_at.is_err() {
            error!(target: "app", "REST - GET - find_all - cannot parse dates");
            continue;
        }

        let online: Online = Online {
            uuid: from_db_key_to_uuid(db_key.as_str()),
            apiToken: api_token.unwrap().to_string(),
            fcmToken: fcm_token.unwrap().to_string(),
            createdAt: created_at.unwrap().to_string(),
            modifiedAt: modified_at.unwrap().to_string(),
        };
        not_online_devices.push(online);
    }
    not_online_devices
}

pub fn from_uuid_to_db_key(uuid: &str) -> String {
    let env = env::var("ENV").ok().unwrap_or("".to_string());
    if env == "testing" { "test-" } else { "online-" }.to_owned() + uuid
}

pub fn from_db_key_to_uuid(db_key: &str) -> String {
    let env = env::var("ENV").ok().unwrap_or("".to_string());
    let pattern = if env == "testing" { "test-" } else { "online-" };
    db_key.replace(pattern, "")
}

pub fn get_all_keys_pattern() -> String {
    let env = env::var("ENV").ok().unwrap_or("".to_string());
    (if env == "testing" { "test-*" } else { "online-*" }).to_owned()
}

pub fn get_date_field_by_name(value: &HashMap<String, String>, field_name: &str) -> Result<u128, DbError> {
    if field_name != "createdAt" || field_name != "modifiedAt" {
        return Err(DbError::UnknownFieldNameError);
    }
    let date: Result<u128, DbError> = match value.get(field_name) {
        Some(val) => match val.parse::<u128>() {
            Ok(val) => Ok(val),
            Err(_) => Err(DbError::DbStrToNumError),
        },
        None => Ok(0u128),
    };
    date
}
