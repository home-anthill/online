use log::{debug, error, info};
use rocket::http::Status;
use rocket::serde::json::{json, Json};
use rocket_db_pools::deadpool_redis::redis::{AsyncCommands, Value};
use rocket_db_pools::Connection;
use std::collections::HashMap;
use std::time::UNIX_EPOCH;

use crate::db::online::{find_all, from_uuid_to_db_key, get_date_field_by_name};
use crate::db::RedisPool;
use crate::errors::api_error::{ApiError, ApiResponse};
use crate::errors::db_error::DbError;
use crate::models::inputs::InitFCMTTokenInput;
use crate::models::online::Online;

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
    info!(target: "app", "REST - GET - get_online called with uuid = {}", uuid);
    let mut con = db.clone();

    let db_key = from_uuid_to_db_key(uuid);
    debug!(target: "app", "REST - GET - get_online - db_key = {:?}", db_key);

    let is_exists: Value = con.exists(&db_key).await.unwrap();
    if is_exists != Value::Int(1) {
        error!(target: "app", "REST - GET - get_online - not found");
        return ApiResponse {
            json: serde_json::to_value(ApiError {
                message: "Not found error".to_string(),
                code: Status::NotFound.code,
            })
            .unwrap(),
            code: Status::NotFound.code,
        };
    }

    let value: HashMap<String, String> = con.hgetall(&db_key).await.unwrap();
    debug!(target: "app", "REST - GET - get_online - value = {:?}", &value);

    let api_token: Result<&str, DbError> = match &value.get("apiToken") {
        Some(val) => Ok(val),
        None => Err(DbError::DbNotFound),
    };
    let created_at: Result<u128, DbError> = get_date_field_by_name(&value, "createdAt");
    let modified_at: Result<u128, DbError> = get_date_field_by_name(&value, "modifiedAt");

    if api_token.is_err() {
        error!(target: "app", "REST - GET - get_online - apiToken is missing");
        return ApiResponse {
            json: serde_json::to_value(ApiError {
                message: "ApiToken is missing in db object".to_string(),
                code: Status::InternalServerError.code,
            })
            .unwrap(),
            code: Status::InternalServerError.code,
        };
    }
    if created_at.is_err() || modified_at.is_err() {
        error!(target: "app", "REST - GET - get_online - cannot parse dates");
        return ApiResponse {
            json: serde_json::to_value(ApiError {
                message: "Cannot parse dates".to_string(),
                code: Status::InternalServerError.code,
            })
            .unwrap(),
            code: Status::InternalServerError.code,
        };
    }
    ApiResponse {
        json: json!({
            "apiToken": api_token.unwrap(),
            "createdAt": created_at.unwrap(),
            "modifiedAt": modified_at.unwrap(),
            "currentTime": UNIX_EPOCH.elapsed().unwrap().as_millis()
        }),
        code: Status::Ok.code,
    }
}

/// init fcm token
#[post("/fcmtoken", data = "<input>")]
pub async fn post_init_fcmtoken(db: Connection<RedisPool>, input: Json<InitFCMTTokenInput>) -> ApiResponse {
    info!(target: "app", "REST - POST - post_init_fcmtoken");
    let mut con = db.clone();

    // get all elements based on apiToken
    let onlines: Vec<Online> = find_all(&con)
        .await
        .into_iter()
        .filter(|o| o.apiToken == input.apiToken)
        .collect();
    for online in &onlines {
        debug!(target: "app", "REST - POST - post_init_fcmtoken - online = {:?}", &online);
        let _: Value = con
            .hset_multiple(
                from_uuid_to_db_key(online.uuid.as_str()),
                &[("fcmToken", input.fcmToken.as_str())],
            )
            .await
            .unwrap();
    }

    ApiResponse {
        json: json!({}),
        code: Status::Ok.code,
    }
}
