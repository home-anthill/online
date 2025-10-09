use std::collections::HashMap;
use std::env;

use futures::StreamExt;
use rocket_db_pools::deadpool_redis::redis::{AsyncCommands, aio::MultiplexedConnection};
use tracing::{error, info};

use crate::errors::db_error::DbError;
use crate::models::online::Online;

pub async fn find_all(db: &MultiplexedConnection) -> Vec<Online> {
    info!(target: "app", "find_all - To get all online elements from db");
    let mut con = db.clone();

    let mut not_online_devices: Vec<Online> = vec![];

    // get all keys with format 'online_<deviceUuid>_feature_<featureUuid>'
    let db_keys: Vec<String> = con
        .scan_match::<&str, String>(get_all_keys_pattern().as_str())
        .await
        .unwrap()
        .collect()
        .await;

    for db_key in db_keys {
        // hgetall returns the entire redis hash table (with all "key: value")
        let value: HashMap<String, String> = con.hgetall(db_key.as_str()).await.unwrap();

        let items: Vec<&str> = db_key.split('_').collect();
        let device_uuid = items.get(1).unwrap().to_string();
        let feature_uuid = items.last().unwrap().to_string();

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
            apiToken: api_token.unwrap().to_string(),
            deviceUuid: device_uuid.to_string(),
            featureUuid: feature_uuid.to_string(),
            fcmToken: fcm_token.unwrap().to_string(),
            createdAt: created_at.unwrap().to_string(),
            modifiedAt: modified_at.unwrap().to_string(),
        };
        not_online_devices.push(online);
    }
    not_online_devices
}

pub fn from_uuid_to_db_key(device_uuid: &str, feature_uuid: &str) -> String {
    let env = env::var("ENV").ok().unwrap_or("".to_string());
    if env == "testing" { "test_" } else { "online_" }.to_owned() + device_uuid + "_feature_" + feature_uuid
}

pub fn get_all_keys_pattern() -> String {
    let env = env::var("ENV").ok().unwrap_or("".to_string());
    (if env == "testing" { "test_*" } else { "online_*" }).to_owned()
}

pub fn get_date_field_by_name(value: &HashMap<String, String>, field_name: &str) -> Result<u128, DbError> {
    if field_name != "createdAt" && field_name != "modifiedAt" {
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
