use log::{debug, error, info};
use mongodb::bson::doc;
use mongodb::Database;
use rocket::http::Status;
use rocket::serde::json::json;
use rocket::State;

use crate::db::online;
use crate::errors::api_error::{ApiError, ApiResponse};

/// keepalive
#[get("/keepalive")]
pub async fn keep_alive() -> ApiResponse {
    ApiResponse {
        json: json!({ "alive": true }),
        code: Status::Ok.code,
    }
}

/// get sensor value by UUID and type
#[get("/online/<uuid>")]
pub async fn get_online(db: &State<Database>, uuid: &str) -> ApiResponse {
    info!(target: "app", "REST - GET - get_online");
    debug!(target: "app", "REST - GET - called with uuid = {}", uuid);
    match online::find_online_by_uuid(db, uuid).await {
        Ok(online_doc) => {
            info!(target: "app", "REST - GET - result online_doc = {}", online_doc);
            let online = online_doc.get_bool("online").unwrap();
            let created_at = online_doc.get_datetime("createdAt").unwrap().timestamp_millis();
            let modified_at = online_doc.get_datetime("modifiedAt").unwrap().timestamp_millis();
            ApiResponse {
                json: json!({
                    "online": online,
                    "createdAt": created_at,
                    "modifiedAt": modified_at,
                }),
                code: Status::Ok.code,
            }
        }
        Err(error) => {
            error!(target: "app", "REST - GET - error {:?}", &error);
            ApiResponse {
                json: serde_json::to_value(ApiError {
                    message: "Internal server error".to_string(),
                    code: error.clone().code,
                })
                .unwrap(),
                code: error.clone().code,
            }
        }
    }
}
