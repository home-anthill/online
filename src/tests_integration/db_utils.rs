use futures::StreamExt;
use std::collections::HashMap;

use rocket_db_pools::deadpool_redis::redis::{aio::MultiplexedConnection, AsyncCommands, Value};

pub async fn drop_all_test_keys(con: &MultiplexedConnection) {
    let mut conn = (*con).clone();
    let values = conn.scan_match::<&str, String>("test-*").await.unwrap();
    let keys: Vec<String> = values.collect().await;
    for key in &keys {
        conn.del::<&str, u128>(key).await.unwrap();
    }
}

pub async fn insert_online(con: &MultiplexedConnection, key: &str, api_token: &str, date: u128) {
    let mut conn = (*con).clone();
    // fill db with a sensor with default zero value
    let _: Value = conn
        .hset_multiple(
            key,
            &[("apiToken", api_token), ("createdAt", date.to_string().as_str())],
        )
        .await
        .unwrap();
    // read from db
    let is_exists: Value = conn.exists(key).await.unwrap();
    assert_eq!(is_exists, Value::Int(1));

    // hgetall returns the entire redis hash table (with all "key: value")
    let value: HashMap<String, String> = conn.hgetall(key).await.unwrap();
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
