use log::{debug, error, info};
use std::collections::HashMap;
use std::env;

use rocket::http::Status;
use rocket::serde::json::json;
use rocket_db_pools::deadpool_redis::redis::{AsyncCommands, Value};
use rocket_db_pools::Connection;

use crate::db::RedisPool;
use crate::errors::api_error::{ApiError, ApiResponse};

/// keepalive
#[get("/keepalive")]
pub async fn keep_alive() -> ApiResponse {
    ApiResponse {
        json: json!({ "alive": true }),
        code: Status::Ok.code,
    }
}

/// get online value by UUID
#[get("/online/<uuid>")]
pub async fn get_online(db: Connection<RedisPool>, uuid: &str) -> ApiResponse {
    info!(target: "app", "REST - GET - get_online");
    debug!(target: "app", "REST - GET - get_online called with uuid = {}", uuid);
    let mut con = db.clone();

    let env = env::var("ENV").ok().unwrap_or("".to_string());
    debug!(target: "app", "env = {:?}", env);

    let key = (if env == "testing" { "test-" } else { "online-" }).to_owned() + uuid;
    debug!(target: "app", "key = {:?}", key);

    let is_exists: Value = con.exists(&key).await.unwrap();
    debug!(target: "app", "is_exists = {:?}", is_exists);

    if is_exists == Value::Int(1) {
        let value: HashMap<String, u64> = con.hgetall(&key).await.unwrap();
        debug!(target: "app", "value = {:?}", value);

        let online: bool = value.get("online").is_some();
        let created_at: u64 = match value.get("createdAt") {
            Some(val) => *val,
            None => 0u64,
        };
        let modified_at: u64 = match value.get("modifiedAt") {
            Some(val) => *val,
            None => 0u64,
        };
        ApiResponse {
            json: json!({
                "online": online,
                "createdAt": created_at,
                "modifiedAt": modified_at,
            }),
            code: Status::Ok.code,
        }
    } else {
        error!(target: "app", "REST - GET - get_online - error");
        ApiResponse {
            json: serde_json::to_value(ApiError {
                message: "Not found error".to_string(),
                code: Status::NotFound.code,
            })
            .unwrap(),
            code: Status::NotFound.code,
        }
    }
}
