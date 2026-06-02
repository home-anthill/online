use rocket::http::Status;
use rocket::serde::json::{Json, json};
use rocket_db_pools::Connection;
use tracing::{error, info};
use uuid::Uuid;

use crate::db::notification::rotate_notification_api_token;
use crate::db::online::{rotate_api_token, update_fcm_token_by_api_token, update_notification_silenced};
use crate::db::{NotificationsRedisPool, RedisPool};
use crate::errors::api_error::ApiResponse;
use crate::models::inputs::{InitFCMTTokenInput, RotateApiTokenInput, UpdateFeatureNotificationInput};

const MAX_FCM_TOKEN_LEN: usize = 512;

fn error_response(status: Status, message: &str) -> ApiResponse {
    ApiResponse { json: json!({ "message": message, "code": status.code }), code: status.code }
}

/// init fcm token
#[rocket::post("/fcmtoken", format = "json", data = "<input>")]
pub async fn post_init_fcmtoken(mut db: Connection<RedisPool>, input: Json<InitFCMTTokenInput>) -> ApiResponse {
    info!(target: "app", "REST - POST - post_init_fcmtoken");

    let api_token = match Uuid::parse_str(&input.api_token) {
        Ok(val) => val.to_string(),
        Err(_) => return error_response(Status::BadRequest, "Invalid apiToken"),
    };
    if input.fcm_token.is_empty() || input.fcm_token.len() > MAX_FCM_TOKEN_LEN {
        return error_response(Status::BadRequest, "Invalid fcmToken");
    }

    if let Err(e) = update_fcm_token_by_api_token(&mut db, &api_token, &input.fcm_token).await {
        error!(target: "app", "REST - POST - post_init_fcmtoken - update failed: {}", e);
        return error_response(Status::InternalServerError, "Database error");
    }

    ApiResponse { json: json!({}), code: Status::Ok.code }
}

/// update per-feature notification silence preference
#[rocket::put("/online/<device_uuid>/features/<feature_uuid>/notifications", format = "json", data = "<input>")]
pub async fn put_feature_notification(
    mut db: Connection<RedisPool>,
    device_uuid: Uuid,
    feature_uuid: Uuid,
    input: Json<UpdateFeatureNotificationInput>,
) -> ApiResponse {
    info!(target: "app", "REST - PUT - put_feature_notification");

    if let Err(e) = update_notification_silenced(
        &mut db,
        &device_uuid.to_string(),
        &feature_uuid.to_string(),
        input.notification_silenced,
    )
    .await
    {
        error!(target: "app", "REST - PUT - put_feature_notification - update failed: {}", e);
        return error_response(Status::InternalServerError, "Database error");
    }

    ApiResponse { json: json!({}), code: Status::Ok.code }
}

/// rotate apiToken references stored in Redis online state
#[rocket::post("/api-token/rotate", format = "json", data = "<input>")]
pub async fn post_rotate_api_token(
    mut db: Connection<RedisPool>,
    mut notifications_db: Connection<NotificationsRedisPool>,
    input: Json<RotateApiTokenInput>,
) -> ApiResponse {
    info!(target: "app", "REST - POST - post_rotate_api_token");

    let old_api_token = match Uuid::parse_str(&input.old_api_token) {
        Ok(val) => val.to_string(),
        Err(_) => return error_response(Status::BadRequest, "Invalid oldApiToken"),
    };
    let new_api_token = match Uuid::parse_str(&input.new_api_token) {
        Ok(val) => val.to_string(),
        Err(_) => return error_response(Status::BadRequest, "Invalid newApiToken"),
    };

    let mut device_features = Vec::with_capacity(input.device_features.len());
    for device_feature in &input.device_features {
        let device_uuid = match Uuid::parse_str(&device_feature.device_uuid) {
            Ok(val) => val.to_string(),
            Err(_) => return error_response(Status::BadRequest, "Invalid deviceUuid"),
        };
        let feature_uuid = match Uuid::parse_str(&device_feature.feature_uuid) {
            Ok(val) => val.to_string(),
            Err(_) => return error_response(Status::BadRequest, "Invalid featureUuid"),
        };
        device_features.push((device_uuid, feature_uuid));
    }

    if let Err(e) = rotate_api_token(&mut db, &old_api_token, &new_api_token, &device_features).await {
        error!(target: "app", "REST - POST - post_rotate_api_token - update failed: {}", e);
        return error_response(Status::InternalServerError, "Database error");
    }
    if let Err(e) = rotate_notification_api_token(&mut notifications_db, &old_api_token, &new_api_token).await {
        error!(target: "app", "REST - POST - post_rotate_api_token - notification history migration failed: {}", e);
        return error_response(Status::InternalServerError, "Database error");
    }

    ApiResponse { json: json!({}), code: Status::Ok.code }
}
