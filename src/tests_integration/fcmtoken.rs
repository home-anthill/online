use std::time::{SystemTime, UNIX_EPOCH};

use super::rocket;
use pretty_assertions::assert_eq;
use rocket::http::Status;
use rocket::local::asynchronous::{Client, LocalRequest, LocalResponse};
use rocket_db_pools::deadpool_redis::{Config, Connection, Runtime, redis::aio::MultiplexedConnection};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::tests_integration::db_utils::{
    delete_cached_fcmtoken_by_api_token, delete_notifications_by_api_token, drop_all_test_keys, get_api_token_by_uuid,
    get_cached_fcmtoken_by_api_token, get_fcmtoken_by_uuid, get_notification_hash, get_notification_ids_by_api_token,
    get_notification_silenced_by_uuid, insert_notification_for_api_token, insert_online, set_fcmtoken_for_online,
};
use online::models::inputs::{
    InitFCMTTokenInput, RotateApiTokenDeviceFeature, RotateApiTokenInput, UpdateFeatureNotificationInput,
};

#[rocket::async_test]
#[test_log::test]
async fn post_fcmtoken() {
    // init
    let client: Client = Client::tracked(rocket()).await.unwrap();
    let cfg: Config = Config::from_url("redis://localhost:6379");
    let pool = cfg.create_pool(Some(Runtime::Tokio1)).unwrap();
    let connection: Connection = pool.get().await.unwrap();
    let con: MultiplexedConnection = connection.clone();

    // cleanup
    drop_all_test_keys(&con).await;

    // inputs
    let device_uuid: String = Uuid::new_v4().to_string();
    let feature_uuid: String = Uuid::new_v4().to_string();
    let db_key: String = "test_".to_owned() + &device_uuid + "_feature_" + &feature_uuid;
    let api_token: String = Uuid::new_v4().to_string();
    let date: u128 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
    delete_cached_fcmtoken_by_api_token(&con, &api_token).await;
    // insert in db
    insert_online(&con, &db_key, &api_token, date).await;

    // test api
    let fcm_token: String = "mocked_fcm_token".to_owned();
    let body = InitFCMTTokenInput { api_token: api_token.clone(), fcm_token: fcm_token.clone() };
    let req: LocalRequest = client.post("/fcmtoken").json(&body);
    let res: LocalResponse = req.dispatch().await;

    // check response status
    assert_eq!(res.status(), Status::Ok);

    // read from db to check if fcmToken has been set
    let fcm_token_db: String = get_fcmtoken_by_uuid(&con, &db_key).await;
    assert_eq!(fcm_token_db, fcm_token);
    let cached_fcm_token: Option<String> = get_cached_fcmtoken_by_api_token(&con, &api_token).await;
    assert_eq!(cached_fcm_token.as_deref(), Some(fcm_token.as_str()));

    // cleanup
    drop_all_test_keys(&con).await;
    delete_cached_fcmtoken_by_api_token(&con, &api_token).await;
}

#[rocket::async_test]
#[test_log::test]
async fn post_fcmtoken_rejects_invalid_api_token() {
    let client: Client = Client::tracked(rocket()).await.unwrap();
    let body = InitFCMTTokenInput { api_token: "not-a-uuid".to_owned(), fcm_token: "mocked_fcm_token".to_owned() };

    let req: LocalRequest = client.post("/fcmtoken").json(&body);
    let res: LocalResponse = req.dispatch().await;

    assert_eq!(res.status(), Status::BadRequest);
    assert_eq!(res.into_json::<Value>().await.unwrap(), json!({ "message": "Invalid apiToken", "code": 400 }));
}

#[rocket::async_test]
#[test_log::test]
async fn post_fcmtoken_rejects_invalid_fcm_token() {
    let client: Client = Client::tracked(rocket()).await.unwrap();
    let body = InitFCMTTokenInput { api_token: Uuid::new_v4().to_string(), fcm_token: String::new() };

    let req: LocalRequest = client.post("/fcmtoken").json(&body);
    let res: LocalResponse = req.dispatch().await;

    assert_eq!(res.status(), Status::BadRequest);
    assert_eq!(res.into_json::<Value>().await.unwrap(), json!({ "message": "Invalid fcmToken", "code": 400 }));
}

#[rocket::async_test]
#[test_log::test]
async fn put_api_token_updates_stale_online_hash_and_fcm_lookup() {
    let client: Client = Client::tracked(rocket()).await.unwrap();
    let cfg: Config = Config::from_url("redis://localhost:6379");
    let pool = cfg.create_pool(Some(Runtime::Tokio1)).unwrap();
    let connection: Connection = pool.get().await.unwrap();
    let con: MultiplexedConnection = connection.clone();

    drop_all_test_keys(&con).await;

    let device_uuid = Uuid::new_v4().to_string();
    let feature_uuid = Uuid::new_v4().to_string();
    let db_key = "test_".to_owned() + &device_uuid + "_feature_" + &feature_uuid;
    let stale_redis_token = Uuid::new_v4().to_string();
    let old_profile_token = Uuid::new_v4().to_string();
    let new_profile_token = Uuid::new_v4().to_string();
    let fcm_token = "mocked_fcm_token".to_owned();
    let date = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();

    insert_online(&con, &db_key, &stale_redis_token, date).await;
    set_fcmtoken_for_online(&con, &db_key, &fcm_token).await;
    delete_cached_fcmtoken_by_api_token(&con, &stale_redis_token).await;
    delete_cached_fcmtoken_by_api_token(&con, &new_profile_token).await;

    let body = RotateApiTokenInput {
        old_api_token: old_profile_token,
        new_api_token: new_profile_token.clone(),
        device_features: vec![RotateApiTokenDeviceFeature {
            device_uuid: device_uuid.clone(),
            feature_uuid: feature_uuid.clone(),
        }],
    };
    let req: LocalRequest = client.put("/api-token").json(&body);
    let res: LocalResponse = req.dispatch().await;

    assert_eq!(res.status(), Status::Ok);
    assert_eq!(get_api_token_by_uuid(&con, &db_key).await, new_profile_token);
    assert_eq!(get_cached_fcmtoken_by_api_token(&con, &stale_redis_token).await, None);
    assert_eq!(get_cached_fcmtoken_by_api_token(&con, &new_profile_token).await.as_deref(), Some(fcm_token.as_str()));

    drop_all_test_keys(&con).await;
    delete_cached_fcmtoken_by_api_token(&con, &new_profile_token).await;
}

#[rocket::async_test]
#[test_log::test]
async fn put_feature_notification_updates_silence_flag() {
    let client: Client = Client::tracked(rocket()).await.unwrap();
    let cfg: Config = Config::from_url("redis://localhost:6379");
    let pool = cfg.create_pool(Some(Runtime::Tokio1)).unwrap();
    let connection: Connection = pool.get().await.unwrap();
    let con: MultiplexedConnection = connection.clone();

    drop_all_test_keys(&con).await;

    let device_uuid = Uuid::new_v4().to_string();
    let feature_uuid = Uuid::new_v4().to_string();
    let db_key = "test_".to_owned() + &device_uuid + "_feature_" + &feature_uuid;

    let body = UpdateFeatureNotificationInput { notification_silenced: true };
    let req: LocalRequest =
        client.put(format!("/online/{device_uuid}/features/{feature_uuid}/notifications")).json(&body);
    let res: LocalResponse = req.dispatch().await;

    assert_eq!(res.status(), Status::Ok);
    assert_eq!(get_notification_silenced_by_uuid(&con, &db_key).await.as_deref(), Some("true"));

    let body = UpdateFeatureNotificationInput { notification_silenced: false };
    let req: LocalRequest =
        client.put(format!("/online/{device_uuid}/features/{feature_uuid}/notifications")).json(&body);
    let res: LocalResponse = req.dispatch().await;

    assert_eq!(res.status(), Status::Ok);
    assert_eq!(get_notification_silenced_by_uuid(&con, &db_key).await.as_deref(), Some("false"));

    drop_all_test_keys(&con).await;
}

#[rocket::async_test]
#[test_log::test]
async fn put_api_token_migrates_notification_history_to_new_api_token() {
    let client: Client = Client::tracked(rocket()).await.unwrap();
    let cfg: Config = Config::from_url("redis://localhost:6379/1");
    let pool = cfg.create_pool(Some(Runtime::Tokio1)).unwrap();
    let connection: Connection = pool.get().await.unwrap();
    let notifications_con: MultiplexedConnection = connection.clone();

    let old_profile_token = Uuid::new_v4().to_string();
    let new_profile_token = Uuid::new_v4().to_string();
    delete_notifications_by_api_token(&notifications_con, &old_profile_token).await;
    delete_notifications_by_api_token(&notifications_con, &new_profile_token).await;

    let old_devices = json!([{
        "deviceUuid": Uuid::new_v4().to_string(),
        "featureUuid": Uuid::new_v4().to_string(),
        "createdAt": 1710000000001u64,
        "modifiedAt": 1710000000002u64,
    }])
    .to_string();
    let new_devices = json!([{
        "deviceUuid": Uuid::new_v4().to_string(),
        "featureUuid": Uuid::new_v4().to_string(),
        "createdAt": 1710000000003u64,
        "modifiedAt": 1710000000004u64,
    }])
    .to_string();
    insert_notification_for_api_token(
        &notifications_con,
        &old_profile_token,
        "test-notification-old-token",
        1717000000000,
        "home anthill",
        "Device is offline",
        &old_devices,
    )
    .await;
    insert_notification_for_api_token(
        &notifications_con,
        &new_profile_token,
        "test-notification-new-token",
        1717000000001,
        "home anthill",
        "Device is offline",
        &new_devices,
    )
    .await;

    let body = RotateApiTokenInput {
        old_api_token: old_profile_token.clone(),
        new_api_token: new_profile_token.clone(),
        device_features: vec![],
    };
    let req: LocalRequest = client.put("/api-token").json(&body);
    let res: LocalResponse = req.dispatch().await;

    assert_eq!(res.status(), Status::Ok);
    assert_eq!(get_notification_ids_by_api_token(&notifications_con, &old_profile_token).await, Vec::<String>::new());
    assert_eq!(
        get_notification_ids_by_api_token(&notifications_con, &new_profile_token).await,
        vec!["test-notification-old-token".to_owned(), "test-notification-new-token".to_owned()]
    );

    let old_notification = get_notification_hash(&notifications_con, "test-notification-old-token").await;
    assert_eq!(old_notification["apiToken"], new_profile_token);
    assert_eq!(old_notification["apiTokens"], format!(r#"["{new_profile_token}"]"#));

    delete_notifications_by_api_token(&notifications_con, &old_profile_token).await;
    delete_notifications_by_api_token(&notifications_con, &new_profile_token).await;
}

#[rocket::async_test]
#[test_log::test]
async fn put_api_token_rejects_invalid_uuids() {
    let client: Client = Client::tracked(rocket()).await.unwrap();
    let valid_uuid = Uuid::new_v4().to_string();

    let cases = [
        (
            RotateApiTokenInput {
                old_api_token: "not-a-uuid".to_owned(),
                new_api_token: valid_uuid.clone(),
                device_features: vec![],
            },
            "Invalid oldApiToken",
        ),
        (
            RotateApiTokenInput {
                old_api_token: valid_uuid.clone(),
                new_api_token: "not-a-uuid".to_owned(),
                device_features: vec![],
            },
            "Invalid newApiToken",
        ),
        (
            RotateApiTokenInput {
                old_api_token: valid_uuid.clone(),
                new_api_token: valid_uuid.clone(),
                device_features: vec![RotateApiTokenDeviceFeature {
                    device_uuid: "not-a-uuid".to_owned(),
                    feature_uuid: valid_uuid.clone(),
                }],
            },
            "Invalid deviceUuid",
        ),
        (
            RotateApiTokenInput {
                old_api_token: valid_uuid.clone(),
                new_api_token: valid_uuid.clone(),
                device_features: vec![RotateApiTokenDeviceFeature {
                    device_uuid: valid_uuid.clone(),
                    feature_uuid: "not-a-uuid".to_owned(),
                }],
            },
            "Invalid featureUuid",
        ),
    ];

    for (body, message) in cases {
        let req: LocalRequest = client.put("/api-token").json(&body);
        let res: LocalResponse = req.dispatch().await;

        assert_eq!(res.status(), Status::BadRequest);
        assert_eq!(res.into_json::<Value>().await.unwrap(), json!({ "message": message, "code": 400 }));
    }
}
