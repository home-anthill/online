use futures::StreamExt;
use rocket_db_pools::deadpool_redis::redis::{aio::MultiplexedConnection, AsyncCommands, Value};
use std::collections::HashMap;

pub async fn drop_all_test_keys(db: &MultiplexedConnection) {
    let mut conn = (*db).clone();
    let values = conn.scan_match::<&str, String>("test-*").await.unwrap();
    let keys: Vec<String> = values.collect().await;
    for key in &keys {
        conn.del::<&str, u128>(key).await.unwrap();
    }
}

pub async fn get_fcmtoken_by_uuid(db: &MultiplexedConnection, db_key: &str) -> String {
    let mut conn = (*db).clone();
    let is_exists: Value = conn.exists(&db_key).await.unwrap();
    if is_exists != Value::Int(1) {
        return "".to_owned();
    }
    conn.hget(&db_key, "fcmToken").await.unwrap()
}

pub async fn insert_online(db: &MultiplexedConnection, db_key: &str, api_token: &str, date: u128) {
    let mut conn = (*db).clone();
    // fill db with a sensor with default zero value
    let _: Value = conn
        .hset_multiple(
            db_key,
            &[("apiToken", api_token), ("createdAt", date.to_string().as_str())],
        )
        .await
        .unwrap();
    // read from db
    let is_exists: Value = conn.exists(db_key).await.unwrap();
    assert_eq!(is_exists, Value::Int(1));

    // hgetall returns the entire redis hash table (with all "db_key: value")
    let value: HashMap<String, String> = conn.hgetall(db_key).await.unwrap();
    let api_tkn: &str = value.get("apiToken").unwrap();
    let created_at: u128 = value.get("createdAt").unwrap().parse::<u128>().unwrap();
    let modified_at = match value.get("modifiedAt") {
        Some(val) => val.parse::<u128>().unwrap(),
        None => 0u128,
    };
    assert_eq!(api_tkn, api_token);
    assert_eq!(created_at, date);
    assert_eq!(modified_at, 0u128); // because only created and not modified
}
