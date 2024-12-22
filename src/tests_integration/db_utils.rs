use futures::StreamExt;
use std::collections::HashMap;

use rocket_db_pools::deadpool_redis::redis::{aio::MultiplexedConnection, AsyncCommands, Value};

pub async fn drop_all_test_keys(con: &MultiplexedConnection) {
    let mut conn = (*con).clone();
    let values = conn.scan_match::<&str, String>("test-*").await.unwrap();
    let keys: Vec<String> = values.collect().await;
    for key in &keys {
        conn.del::<&str, u64>(key).await.unwrap();
    }
}

pub async fn insert_online(con: &MultiplexedConnection, key: &str, date: u64) {
    let mut conn = (*con).clone();
    // fill db with a sensor with default zero value
    let _: Value = conn
        .hset_multiple(key, &[("online", 1u64), ("createdAt", date)])
        .await
        .unwrap();
    // read from db
    let is_exists: Value = conn.exists(key).await.unwrap();
    assert_eq!(is_exists, Value::Int(1));

    // hgetall returns the entire redis hash table (with all "key: value")
    let value: HashMap<String, u64> = conn.hgetall(key).await.unwrap();
    let online: bool = value.get("online").is_some();
    let created_at: u64 = match value.get("createdAt") {
        Some(val) => *val,
        None => 0u64,
    };
    let modified_at: u64 = match value.get("modifiedAt") {
        Some(val) => *val,
        None => 0u64,
    };
    assert!(online);
    assert_eq!(created_at, date);
    assert_eq!(modified_at, 0); // because only created and not modified
}
