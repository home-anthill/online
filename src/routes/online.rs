use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use rocket::http::Status;
use rocket::serde::json::{Json, json};
use rocket_db_pools::Connection;
use rocket_db_pools::deadpool_redis::redis::{AsyncCommands, pipe};
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::db::OnlineRedisPool;
use crate::db::online::{from_uuid_to_db_key, get_date_field_by_name};
use crate::errors::api_error::ApiResponse;
use crate::models::inputs::OnlineBulkInput;
use crate::models::online::{OnlineBulkResponse, OnlineBulkStatus, OnlineStatus};

const MAX_BULK_DEVICE_FEATURES: usize = 100;

fn error_response(status: Status, message: &str) -> ApiResponse {
    ApiResponse { json: json!({ "message": message, "code": status.code }), code: status.code }
}

/// Get online records for multiple device/feature pairs using one Redis pipeline.
#[rocket::post("/online/bulk", format = "json", data = "<input>")]
pub async fn post_online_bulk(mut db: Connection<OnlineRedisPool>, input: Json<OnlineBulkInput>) -> ApiResponse {
    info!(target: "app", "REST - POST - post_online_bulk called");

    if input.device_features.len() > MAX_BULK_DEVICE_FEATURES {
        return error_response(Status::BadRequest, "Too many device features");
    }

    if input.device_features.is_empty() {
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        let response = OnlineBulkResponse { statuses: vec![], current_time };
        return ApiResponse { json: json!(response), code: Status::Ok.code };
    }

    let keys: Vec<String> = input
        .device_features
        .iter()
        .map(|item| from_uuid_to_db_key(&item.device_uuid.to_string(), &item.feature_uuid.to_string()))
        .collect();

    let mut redis_pipeline = pipe();
    for key in &keys {
        redis_pipeline.cmd("HGETALL").arg(key);
    }

    let values: Vec<HashMap<String, String>> = match redis_pipeline.query_async(&mut *db).await {
        Ok(values) => values,
        Err(e) => {
            error!(target: "app", "REST - POST - post_online_bulk - Redis pipeline error: {}", e);
            return error_response(Status::InternalServerError, "Database error");
        }
    };

    let statuses = input
        .device_features
        .iter()
        .zip(values.iter())
        .map(|(item, value)| {
            let timestamps = if value.contains_key("apiToken") {
                get_date_field_by_name(value, "createdAt").ok().zip(get_date_field_by_name(value, "modifiedAt").ok())
            } else {
                None
            };

            match timestamps {
                Some((created_at, modified_at)) => OnlineBulkStatus {
                    device_uuid: item.device_uuid,
                    feature_uuid: item.feature_uuid,
                    status: OnlineStatus::Found,
                    created_at: Some(created_at),
                    modified_at: Some(modified_at),
                },
                None => OnlineBulkStatus {
                    device_uuid: item.device_uuid,
                    feature_uuid: item.feature_uuid,
                    status: OnlineStatus::Missing,
                    created_at: None,
                    modified_at: None,
                },
            }
        })
        .collect();

    let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    let response = OnlineBulkResponse { statuses, current_time };

    match serde_json::to_value(response) {
        Ok(json) => ApiResponse { json, code: Status::Ok.code },
        Err(e) => {
            error!(target: "app", "REST - POST - post_online_bulk - response serialization error: {}", e);
            error_response(Status::InternalServerError, "Cannot serialize response")
        }
    }
}

/// get online value by UUID
#[rocket::get("/online/<device_uuid>/features/<feature_uuid>")]
pub async fn get_online(mut db: Connection<OnlineRedisPool>, device_uuid: Uuid, feature_uuid: Uuid) -> ApiResponse {
    info!(target: "app", "REST - GET - get_online called");

    let db_key = from_uuid_to_db_key(&device_uuid.to_string(), &feature_uuid.to_string());
    debug!(target: "app", "REST - GET - get_online - db_key = {:?}", db_key);

    let is_exists: bool = match db.exists(&db_key).await {
        Ok(val) => val,
        Err(e) => {
            error!(target: "app", "REST - GET - get_online - Redis exists error: {}", e);
            return error_response(Status::InternalServerError, "Database error");
        }
    };
    if !is_exists {
        error!(target: "app", "REST - GET - get_online - not found");
        return error_response(Status::NotFound, "Not found error");
    }

    let value: HashMap<String, String> = match db.hgetall(&db_key).await {
        Ok(val) => val,
        Err(e) => {
            error!(target: "app", "REST - GET - get_online - Redis hgetall error: {}", e);
            return error_response(Status::InternalServerError, "Database error");
        }
    };
    debug!(target: "app", "REST - GET - get_online - value retrieved");

    // Validate that required fields are present; return 404 to avoid revealing
    // whether a key exists but has corrupt data.
    if !value.contains_key("apiToken") {
        error!(target: "app", "REST - GET - get_online - apiToken field missing");
        return error_response(Status::NotFound, "Not found");
    }
    let created_at = match get_date_field_by_name(&value, "createdAt") {
        Ok(val) => val,
        Err(_) => {
            error!(target: "app", "REST - GET - get_online - cannot parse createdAt");
            return error_response(Status::NotFound, "Not found");
        }
    };
    let modified_at = match get_date_field_by_name(&value, "modifiedAt") {
        Ok(val) => val,
        Err(_) => {
            error!(target: "app", "REST - GET - get_online - cannot parse modifiedAt");
            return error_response(Status::NotFound, "Not found");
        }
    };

    let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();

    ApiResponse {
        json: json!({
            "createdAt": created_at,
            "modifiedAt": modified_at,
            "currentTime": current_time
        }),
        code: Status::Ok.code,
    }
}

/// delete online to prevent infinite notifications
#[rocket::delete("/online/<device_uuid>/features/<feature_uuid>")]
pub async fn delete_online(mut db: Connection<OnlineRedisPool>, device_uuid: Uuid, feature_uuid: Uuid) -> ApiResponse {
    info!(target: "app", "REST - DELETE - delete_online called");

    let db_key = from_uuid_to_db_key(&device_uuid.to_string(), &feature_uuid.to_string());
    debug!(target: "app", "REST - DELETE - delete_online - db_key = {:?}", db_key);

    let is_exists: bool = match db.exists(&db_key).await {
        Ok(val) => val,
        Err(e) => {
            error!(target: "app", "REST - DELETE - delete_online - Redis exists error: {}", e);
            return error_response(Status::InternalServerError, "Database error");
        }
    };
    if !is_exists {
        info!(target: "app", "REST - DELETE - delete_online - already don't exist");
        return ApiResponse { json: json!({}), code: Status::Ok.code };
    }

    let res: u64 = match db.del(&db_key).await {
        Ok(val) => val,
        Err(e) => {
            error!(target: "app", "REST - DELETE - delete_online - Redis del error: {}", e);
            return error_response(Status::InternalServerError, "Database error");
        }
    };
    if res != 1 {
        error!(target: "app", "REST - DELETE - delete_online - cannot delete online");
        return error_response(Status::InternalServerError, "Cannot delete error");
    }
    ApiResponse { json: json!({}), code: Status::Ok.code }
}
