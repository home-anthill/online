use futures::StreamExt;
use pretty_assertions::assert_eq;
use rocket_db_pools::deadpool_redis::redis::{AsyncCommands, Value, aio::MultiplexedConnection};
use std::collections::HashMap;

const FCM_BY_API_TOKEN_KEY: &str = "fcm_by_api_token";

pub async fn drop_all_test_keys(db: &MultiplexedConnection) {
    let mut conn = (*db).clone();
    let values = conn.scan_match::<&str, String>("test_*").await.unwrap();
    let keys: Vec<String> = values.collect().await;
    for key in &keys {
        conn.del::<&str, u128>(key).await.unwrap();
    }
}

pub async fn get_fcmtoken_by_uuid(db: &MultiplexedConnection, db_key: &str) -> String {
    let mut conn = (*db).clone();
    let is_exists: Value = conn.exists(db_key).await.unwrap();
    if is_exists != Value::Int(1) {
        return "".to_owned();
    }
    conn.hget(db_key, "fcmToken").await.unwrap()
}

pub async fn get_api_token_by_uuid(db: &MultiplexedConnection, db_key: &str) -> String {
    let mut conn = (*db).clone();
    let is_exists: Value = conn.exists(db_key).await.unwrap();
    if is_exists != Value::Int(1) {
        return "".to_owned();
    }
    conn.hget(db_key, "apiToken").await.unwrap()
}

pub async fn get_cached_fcmtoken_by_api_token(db: &MultiplexedConnection, api_token: &str) -> Option<String> {
    let mut conn = (*db).clone();
    conn.hget(FCM_BY_API_TOKEN_KEY, api_token).await.unwrap()
}

pub async fn delete_cached_fcmtoken_by_api_token(db: &MultiplexedConnection, api_token: &str) {
    let mut conn = (*db).clone();
    conn.hdel::<_, _, u128>(FCM_BY_API_TOKEN_KEY, api_token).await.unwrap();
}

pub async fn insert_online(db: &MultiplexedConnection, db_key: &str, api_token: &str, date: u128) {
    let mut conn = (*db).clone();
    // fill db with a sensor with default zero value
    let date = date.to_string();
    let _: Value = conn
        .hset_multiple(db_key, &[("apiToken", api_token), ("createdAt", date.as_str()), ("modifiedAt", date.as_str())])
        .await
        .unwrap();
    // read from db
    let is_exists: Value = conn.exists(db_key).await.unwrap();
    assert_eq!(is_exists, Value::Int(1));

    // hgetall returns the entire redis hash table (with all "db_key: value")
    let value: HashMap<String, String> = conn.hgetall(db_key).await.unwrap();
    let api_tkn: &str = value.get("apiToken").unwrap();
    let created_at: u128 = value.get("createdAt").unwrap().parse::<u128>().unwrap();
    let modified_at: u128 = value.get("modifiedAt").unwrap().parse::<u128>().unwrap();
    assert_eq!(api_tkn, api_token);
    assert_eq!(created_at, date.parse::<u128>().unwrap());
    assert_eq!(modified_at, created_at);
}

pub async fn insert_online_fields(db: &MultiplexedConnection, db_key: &str, fields: &[(&str, &str)]) {
    let mut conn = (*db).clone();
    conn.hset_multiple::<_, _, _, ()>(db_key, fields).await.unwrap();

    let is_exists: Value = conn.exists(db_key).await.unwrap();
    assert_eq!(is_exists, Value::Int(1));
}

pub async fn set_fcmtoken_for_online(db: &MultiplexedConnection, db_key: &str, fcm_token: &str) {
    let mut conn = (*db).clone();
    conn.hset::<_, _, _, ()>(db_key, "fcmToken", fcm_token).await.unwrap();
}
