use rocket::http::Status;
use rocket::serde::json::json;
use rocket_db_pools::Connection;
use tracing::{error, info};
use uuid::Uuid;

use crate::db::NotificationsRedisPool;
use crate::db::notification::list_profile_notifications;
use crate::errors::api_error::ApiResponse;

fn error_response(status: Status, message: &str) -> ApiResponse {
    ApiResponse { json: json!({ "message": message, "code": status.code }), code: status.code }
}

/// list notifications for a profile apiToken
#[rocket::get("/notifications/<api_token>")]
pub async fn get_profile_notifications(mut db: Connection<NotificationsRedisPool>, api_token: &str) -> ApiResponse {
    info!(target: "app", "REST - GET - get_profile_notifications called");

    let api_token = match Uuid::parse_str(api_token) {
        Ok(val) => val.to_string(),
        Err(_) => return error_response(Status::BadRequest, "Invalid apiToken"),
    };

    let notifications = match list_profile_notifications(&mut db, &api_token).await {
        Ok(val) => val,
        Err(e) => {
            error!(target: "app", "REST - GET - get_profile_notifications - read failed: {}", e);
            return error_response(Status::InternalServerError, "Database error");
        }
    };

    ApiResponse { json: json!({ "notifications": notifications }), code: Status::Ok.code }
}
